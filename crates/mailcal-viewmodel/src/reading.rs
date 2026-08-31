//! The reading-view snapshot: the displayable body of the one open message.

use crate::{avatar::Avatar, invitation::InvitationCard};

/// One downloadable attachment shown below an open message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttachmentRow {
    /// Message-scoped attachment id; pass it back to the core to save/download this part.
    pub id: u32,
    /// Suggested display/download file name.
    pub file_name: String,
    /// Media type, e.g. `application/pdf`.
    pub media_type: String,
    /// Decoded byte length.
    pub size: u64,
}

/// An immutable snapshot of an open message's body for a host to render.
///
/// `html` is **already sanitised**: the product core strips the engine's raw, hostile
/// `text/html` to a safe, inert subset (scripts, event handlers, and frames removed) while
/// **preserving presentational CSS**, and flags remote references in `has_remote_images`.
/// A host must not render `html` directly: wrap it with `mailcal_app::render_document`
/// (FFI `render_message_html`), which is the security boundary; its strict CSP blocks
/// scripts and, by default, every remote load (so remote images don't load until the user
/// opts in). Render the result in a WebView with scripting off and navigation blocked, or
/// fall back to `plain`. Both `None` means no text body, or the source could not be fetched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadingSnapshot {
    /// The provider key of the message this body is for, so a host can match it to the
    /// row it opened and ignore a stale snapshot (the default is the empty key, before
    /// any message has been opened).
    pub key: String,
    /// The sender, formatted for display (`Name <email>`, or bare `email` when the header
    /// carried no name); empty when there's no sender. Shown in the reading header: the full
    /// `Name <email>`, unlike the list row's name-only [`crate::FlatRow::from`].
    pub from: String,
    /// The sender's monogram, colour and photo, beside the sender in the reading header.
    pub avatar: Avatar,
    /// The `To` recipients, formatted for display (`Name <email>` or bare `email`) and
    /// comma-joined; empty when none. Shown in the reading header.
    pub to: String,
    /// The `Cc` recipients, formatted and comma-joined; empty when none.
    pub cc: String,
    /// The `Bcc` recipients, formatted and comma-joined; empty when none. Present only for a
    /// message whose stored copy carries a `Bcc` header; i.e. the sender's own Sent/Drafts
    /// copy: so the sender can see whom they Bcc'd; a received message never carries it.
    pub bcc: String,
    /// The sanitised HTML body, when the message carries an HTML part. Presentational CSS
    /// is preserved; a host renders this via `mailcal_app::render_document` (which wraps it
    /// in a strict-CSP document) inside a WebView with scripting off and navigation blocked.
    pub html: Option<String>,
    /// The plain-text body, when the message carries one.
    pub plain: Option<String>,
    /// Whether the HTML references a remote resource (a remote image or CSS background) that
    /// is blocked by default. When `true`, a host offers a "load remote images" confirmation
    /// and, on accept, re-renders with `render_document(.., load_remote_images = true)`.
    pub has_remote_images: bool,
    /// Whether the body could not be **fetched** (provider/network error), as distinct from
    /// a message that genuinely has no body (`html`/`plain` both `None`, `load_error` false).
    /// A host shows a "couldn't load; retry" affordance for the former, not the latter.
    pub load_error: bool,
    /// Downloadable attachments decoded from the message source. Empty when none or when the
    /// body failed to load.
    ///
    /// A meeting invitation's `text/calendar` payload is deliberately **not** here: it is an
    /// alternative body part, consumed into [`Self::invitation`], and showing it as a file
    /// would put a paperclip on every invitation that nobody sent. A calendar file the sender
    /// explicitly *attached* (Gmail's duplicate `invite.ics`, a published `.ics`) still appears.
    pub attachments: Vec<AttachmentRow>,
    /// The meeting-invitation card, when this message carries an iTIP scheduling object that
    /// warrants one; `None` for ordinary mail.
    ///
    /// A host draws it **above** the body. Its text fields are attacker-controlled plain text;
    /// see [`crate::invitation::InvitationCard`].
    pub invitation: Option<InvitationCard>,
    /// The open for [`Self::key`] is still running, and has been long enough to be worth
    /// saying so: a host shows its loading indicator, and only then.
    ///
    /// There is no body on a snapshot carrying this; it exists to announce a wait, not to
    /// deliver anything. A host that has no snapshot for the key it opened is in the *same*
    /// state, minus the announcement, and draws the body area empty; the header it already has
    /// from the list row keeps the pane from reading as broken. The core decides when a wait
    /// has gone on long enough, so the answer is the same on every platform and a fast open
    /// never flashes a spinner it immediately takes away.
    pub pending: bool,
}
