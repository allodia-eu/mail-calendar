//! Sharing into the app: another application hands us files, text, or both, and asks for a mail
//! client. The shared decode behind "share this by email".
//!
//! The twin of [`crate::mailto`], and deliberately the same shape: pure, host-free, and the whole
//! of a client's decision-making. A wrapper's job is to convert what its OS hands it into a
//! [`ShareRequest`], call [`prefill_from_share`], and open a composer with the answer. Nothing
//! here reads a file, allocates a draft, or sends.
//!
//! Four rules are load-bearing (`docs/os-integration.md`, and Gate 13 in
//! `docs/composer-security.md`):
//!
//! - **A share opens an editable composer and never sends.** The user is always the one who sends,
//!   exactly as with a mail link.
//! - **Names and media types are hostile input**, normalised by [`crate::file_meta`]. They were
//!   chosen by the sharing app, not by us and not by the user.
//! - **Nothing is dropped silently.** An item we refuse comes back in [`SharePrefill::rejected`]
//!   with a reason, because a file the user watched disappear from a share sheet is one they will
//!   assume was attached.
//! - **A share carries no recipients of its own.** The only route to a pre-filled `To` is shared
//!   *text* that is itself a `mailto:` link, which is then decoded by the same allowlist every mail
//!   link goes through. A sharing app cannot address a message.
//!
//! ⚠️ **Attachments never come from a URI.** `mailto:?attach=` is not RFC 6068, and a handler
//! cannot tell a URI that came from `xdg-email` from one that came from a web page, so honouring
//! it would let a page attach any local file it could name. Files reach this module only from a
//! channel that is itself a user action: a share sheet, an "Open With", an explicit `--attach`.

use core::fmt;

use crate::{
    file_meta::{safe_file_name, safe_media_type},
    mailto::parse_mailto,
};

/// The most files one share may seed a composer with.
///
/// A share sheet is a deliberate selection, so this guards against a misbehaving sender rather
/// than limiting anyone: multi-select pickers stop well below it. Over the cap the **first**
/// items are kept, since a selection's order is the user's own.
pub const MAX_SHARED_ITEMS: usize = 20;

/// One thing another application handed us.
///
/// `path` is a local file the core can read at submit time. A client that receives a handle
/// rather than a path (an Android `content://` URI, a Windows `StorageFile`, an
/// `NSItemProvider`) stages the bytes to its own private storage first: this crate never learns
/// what the original handle was.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SharedItem {
    /// Where the bytes are, on this device, now.
    pub path: String,
    /// The display name the sharing app offered, if it offered one (may be empty).
    pub suggested_name: String,
    /// The media type the sharing app declared, if it declared one (may be empty).
    pub declared_media_type: String,
}

/// What a share asked the composer to open with, before the core has had its say.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ShareRequest {
    /// The files, in the order the user selected them.
    pub items: Vec<SharedItem>,
    /// Text the share carried: a selection, a URL, or a whole `mailto:` link (may be empty).
    pub text: String,
    /// A subject the sharing app suggested (may be empty). Android's `EXTRA_SUBJECT`, and the
    /// page title a browser shares alongside a link.
    pub subject: String,
}

/// One file the composer will open with, named and typed safely.
///
/// The same three fields the composer's own file picker produces, so a seeded attachment and a
/// picked one are indistinguishable by the time either is submitted.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ShareAttachment {
    /// Where the bytes are; read by the core at submit, never across FFI.
    pub path: String,
    /// The name the outgoing MIME part carries.
    pub file_name: String,
    /// The media type the outgoing MIME part carries.
    pub media_type: String,
}

/// Why one shared item did not become an attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareRejection {
    /// The item carried no path, so there are no bytes to attach.
    NoPath,
    /// The same path appeared earlier in the same share.
    Duplicate,
    /// The share carried more than [`MAX_SHARED_ITEMS`] files.
    TooMany,
}

/// An item that did not become an attachment, and why, so a client can say so.
#[derive(Clone, PartialEq, Eq)]
pub struct RejectedItem {
    /// The item's name, normalised the same way an accepted one's would be, so it is safe to
    /// show and reads as the file the user chose.
    pub name: String,
    /// Why it was refused.
    pub reason: ShareRejection,
}

/// What the composer opens with after a share.
///
/// The recipient and text fields are exactly [`crate::MailtoPrefill`]'s, so a client that
/// already opens a composer from a mail link seeds one from a share through the same code.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SharePrefill {
    /// The `To` recipients, comma-separated. Non-empty only when the shared text was a
    /// `mailto:` link.
    pub to: String,
    /// The `Cc` recipients, comma-separated (may be empty).
    pub cc: String,
    /// The `Bcc` recipients, comma-separated (may be empty).
    pub bcc: String,
    /// The suggested subject (may be empty).
    pub subject: String,
    /// The suggested plain-text body, one paragraph per line (may be empty).
    pub body: String,
    /// The files to seed the composer's attachment list with, in the user's own order.
    pub attachments: Vec<ShareAttachment>,
    /// What was refused, and why. Empty in the ordinary case.
    pub rejected: Vec<RejectedItem>,
}

impl SharePrefill {
    /// Whether the share carried nothing usable: no attachment and no text.
    ///
    /// A client can use this to decide between "open the composer pre-filled" and ignoring the
    /// launch. It is **not** the same question as "was anything refused": a share of one
    /// unreadable file is empty *and* has a rejection to report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty()
            && self.to.is_empty()
            && self.cc.is_empty()
            && self.bcc.is_empty()
            && self.subject.is_empty()
            && self.body.is_empty()
    }
}

/// Turns what an OS share handed a client into composer prefill.
///
/// Files are accepted in order, skipping any without a path and any repeat of a path already
/// taken, until [`MAX_SHARED_ITEMS`] have been accepted; everything refused is reported in
/// [`SharePrefill::rejected`].
///
/// The text is read once, and it decides the message fields:
///
/// - text that is a **`mailto:` link** is decoded by [`parse_mailto`], so a share and a tapped mail
///   link honour one header allowlist. Its subject and body win over the share's own, being the
///   more specific request.
/// - any other text becomes the **body**: a shared URL, a quoted selection, a note.
///
/// Pure and synchronous: no store, no network, no account, so a host may call it during a cold
/// launch before the core has finished connecting.
#[must_use]
pub fn prefill_from_share(request: ShareRequest) -> SharePrefill {
    let mut prefill = SharePrefill::default();

    for item in request.items {
        let name = safe_file_name(&item.suggested_name, &item.path);
        if item.path.trim().is_empty() {
            prefill.rejected.push(RejectedItem {
                name,
                reason: ShareRejection::NoPath,
            });
        } else if prefill
            .attachments
            .iter()
            .any(|accepted| accepted.path == item.path)
        {
            prefill.rejected.push(RejectedItem {
                name,
                reason: ShareRejection::Duplicate,
            });
        } else if prefill.attachments.len() >= MAX_SHARED_ITEMS {
            prefill.rejected.push(RejectedItem {
                name,
                reason: ShareRejection::TooMany,
            });
        } else {
            prefill.attachments.push(ShareAttachment {
                media_type: safe_media_type(&item.declared_media_type, &name),
                file_name: name,
                path: item.path,
            });
        }
    }

    // A shared `mailto:` link is the one way a share reaches the recipient fields, and it goes
    // through the same decode a tapped link does rather than a second, laxer one.
    if let Some(link) = parse_mailto(&request.text) {
        prefill.to = link.to;
        prefill.cc = link.cc;
        prefill.bcc = link.bcc;
        prefill.subject = link.subject;
        prefill.body = link.body;
    } else {
        prefill.body = body_text(&request.text);
    }
    if prefill.subject.is_empty() {
        prefill.subject = header_text(&request.subject);
    }

    prefill
}

/// Normalises shared text into a body: line endings collapse to `\n` and every other control
/// character is dropped. The twin of the mail-link body rule, so the same text seeds the same
/// composer whichever way it arrived.
fn body_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

/// Normalises a shared subject: a single line, so a newline in it cannot start a header of the
/// sharing app's choosing when the draft is assembled.
fn header_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_owned()
}

// Everything a share carries is message content: the names of the user's files, their text, who
// they are writing to. So all four types redact the same way the rest of this crate does,
// lengths and counts only, and stay safe to put in a diagnostic log line (`docs/logging.md`).
impl fmt::Debug for SharedItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedItem")
            .field("has_path", &!self.path.is_empty())
            .field("has_suggested_name", &!self.suggested_name.is_empty())
            .field("declared_media_type", &self.declared_media_type)
            .finish()
    }
}

impl fmt::Debug for ShareRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareRequest")
            .field("items", &self.items.len())
            .field("text_len", &self.text.len())
            .field("subject_len", &self.subject.len())
            .finish()
    }
}

impl fmt::Debug for ShareAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareAttachment")
            // The path is a staged file named after the user's own, so it is content too.
            .field("has_path", &!self.path.is_empty())
            .field("file_name_len", &self.file_name.len())
            .field("media_type", &self.media_type)
            .finish()
    }
}

impl fmt::Debug for RejectedItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectedItem")
            .field("name_len", &self.name.len())
            .field("reason", &self.reason)
            .finish()
    }
}

impl fmt::Debug for SharePrefill {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharePrefill")
            .field("to_len", &self.to.len())
            .field("cc_len", &self.cc.len())
            .field("bcc_len", &self.bcc.len())
            .field("subject_len", &self.subject.len())
            .field("body_len", &self.body.len())
            .field("attachments", &self.attachments.len())
            .field("rejected", &self.rejected.len())
            .finish()
    }
}
