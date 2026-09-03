//! FFI surface for a share: another app hands us files or text and asks for a mail client.
//!
//! The twin of [`crate::mailto`], and the same division of labour. The OS hands a client some
//! items; the client asks Rust what they mean and opens its composer with the answer. The
//! decode lives in `mailcal_composer::share` so every platform inherits one filename and
//! media-type policy, one cap, and one rule about what a share may and may not pre-fill; see
//! `docs/os-integration.md` and `docs/composer-security.md`, Gate 13.

use mailcal_composer::{ShareRejection as CoreRejection, ShareRequest as CoreRequest, SharedItem};

use crate::ComposerFileAttachment;

/// One thing another application shared with us.
///
/// `path` must be a file **this app can read**: a host holding an OS handle instead (an Android
/// `content://` URI, a Windows `StorageFile`, an `NSItemProvider`) stages the bytes into its own
/// private storage first and passes that path. The core never learns what the original handle
/// was, and never reads the file here: the bytes are read at submit, by
/// [`crate::MailcalApp::submit_rich_mail_with_files`], and do not cross FFI.
#[derive(uniffi::Record)]
pub struct SharedFile {
    /// Where the bytes are, on this device, now.
    pub path: String,
    /// The display name the sharing app offered, or blank. Preferred over the path's own name,
    /// which for a staged file is a temporary nobody would recognise.
    pub suggested_name: String,
    /// The media type the sharing app declared, or blank. Trusted only when it is well formed;
    /// a wildcard (what Android hands a target that accepts anything) is not.
    pub declared_media_type: String,
}

/// What an OS share handed a client, before the core has had its say.
#[derive(uniffi::Record)]
pub struct ShareRequest {
    /// The files, in the order the user selected them.
    pub files: Vec<SharedFile>,
    /// Text the share carried: a selection, a URL, or a whole `mailto:` link. May be blank.
    pub text: String,
    /// A subject the sharing app suggested (Android's `EXTRA_SUBJECT`, a browser's page title).
    /// May be blank.
    pub subject: String,
}

/// Why one shared file did not become an attachment, so a host can say so rather than leave the
/// user to notice.
#[derive(uniffi::Enum)]
pub enum ShareRejectionReason {
    /// No path, so there are no bytes to attach.
    NoPath,
    /// The same file appeared twice in one share.
    Duplicate,
    /// The share carried more files than one message may seed.
    TooMany,
}

/// A shared file that was refused, and why.
#[derive(uniffi::Record)]
pub struct RejectedShare {
    /// The file's name, normalised exactly as an accepted one's would be, so it is safe to put
    /// on screen and reads as the file the user chose.
    pub name: String,
    /// Why it was refused.
    pub reason: ShareRejectionReason,
}

/// What the composer opens with after a share.
///
/// The five text fields are [`crate::MailtoPrefill`]'s, in the same shape, so a client that
/// already opens a composer from a mail link seeds one from a share through the same code.
/// `attachments` are [`ComposerFileAttachment`]s, which is what
/// [`crate::MailcalApp::submit_rich_mail_with_files`] already consumes, so a seeded attachment
/// and a picked one are indistinguishable by the time either is sent.
///
/// **Nothing here is ever sent**: a share pre-fills an editable composer, and the user is still
/// the one who sends it.
#[derive(uniffi::Record)]
pub struct SharePrefill {
    /// The `To` recipients, comma-separated. Non-blank **only** when the shared text was itself
    /// a `mailto:` link: a sharing app cannot otherwise address a message.
    pub to: String,
    /// The `Cc` recipients, comma-separated (may be blank).
    pub cc: String,
    /// The `Bcc` recipients, comma-separated (may be blank).
    pub bcc: String,
    /// The suggested subject (may be blank).
    pub subject: String,
    /// The suggested plain-text body, one paragraph per line (may be blank).
    pub body: String,
    /// The files to seed the composer's attachment list with, in the user's own order.
    pub attachments: Vec<ComposerFileAttachment>,
    /// What was refused, and why. Empty in the ordinary case.
    pub rejected: Vec<RejectedShare>,
    /// Whether the share carried nothing usable, so a host can ignore the launch instead of
    /// opening a blank composer over whatever the user was doing. Note this is a **different**
    /// question from whether anything was refused: a share of one unreadable file is empty and
    /// still has something to report.
    pub is_empty: bool,
}

/// Turns an OS share into composer prefill.
///
/// Names and media types are normalised (a name that is a path keeps only its final component;
/// control characters and bidirectional overrides are dropped; a wildcard media type falls back
/// to what the extension implies), files past the cap and repeats of one already taken come back in
/// `rejected`, and shared text that is a `mailto:` link is decoded through the same header
/// allowlist a tapped link goes through.
///
/// Pure and synchronous: no store, no network, no account, and no file is opened, so a host may
/// call it during a cold launch before the core has finished connecting.
#[must_use]
#[uniffi::export]
pub fn prefill_from_share(request: ShareRequest) -> SharePrefill {
    let prefill = mailcal_composer::prefill_from_share(CoreRequest {
        items: request
            .files
            .into_iter()
            .map(|file| SharedItem {
                path: file.path,
                suggested_name: file.suggested_name,
                declared_media_type: file.declared_media_type,
            })
            .collect(),
        text: request.text,
        subject: request.subject,
    });

    SharePrefill {
        is_empty: prefill.is_empty(),
        to: prefill.to,
        cc: prefill.cc,
        bcc: prefill.bcc,
        subject: prefill.subject,
        body: prefill.body,
        attachments: prefill
            .attachments
            .into_iter()
            .map(|attachment| ComposerFileAttachment {
                path: attachment.path,
                file_name: attachment.file_name,
                media_type: attachment.media_type,
            })
            .collect(),
        rejected: prefill
            .rejected
            .into_iter()
            .map(|item| RejectedShare {
                name: item.name,
                reason: match item.reason {
                    CoreRejection::NoPath => ShareRejectionReason::NoPath,
                    CoreRejection::Duplicate => ShareRejectionReason::Duplicate,
                    CoreRejection::TooMany => ShareRejectionReason::TooMany,
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ShareRejectionReason, ShareRequest, SharedFile, prefill_from_share};

    fn file(path: &str, name: &str, media_type: &str) -> SharedFile {
        SharedFile {
            path: path.to_owned(),
            suggested_name: name.to_owned(),
            declared_media_type: media_type.to_owned(),
        }
    }

    #[test]
    fn a_shared_file_crosses_the_ffi_as_a_composer_attachment() {
        // The point of the conversion: what comes back is the exact type
        // `submit_rich_mail_with_files` already takes, so a host seeds its attachment list with
        // it and its send path is unchanged.
        let prefill = prefill_from_share(ShareRequest {
            files: vec![file("/cache/stage-1", "report.pdf", "application/pdf")],
            text: String::new(),
            subject: "Quarterly".to_owned(),
        });

        assert_eq!(prefill.attachments.len(), 1);
        assert_eq!(prefill.attachments[0].path, "/cache/stage-1");
        assert_eq!(prefill.attachments[0].file_name, "report.pdf");
        assert_eq!(prefill.attachments[0].media_type, "application/pdf");
        assert_eq!(prefill.subject, "Quarterly");
        assert!(!prefill.is_empty);
    }

    #[test]
    fn a_share_cannot_address_a_message_by_itself() {
        // The gate is enforced in the shared core, so it holds on every platform; a client
        // cannot widen it by shaping the request differently.
        let prefill = prefill_from_share(ShareRequest {
            files: Vec::new(),
            text: "ada@example.test".to_owned(),
            subject: String::new(),
        });

        assert_eq!(prefill.to, "");
        assert_eq!(prefill.body, "ada@example.test");
    }

    #[test]
    fn shared_text_that_is_a_mail_link_reaches_the_recipient_fields() {
        let prefill = prefill_from_share(ShareRequest {
            files: Vec::new(),
            text: "mailto:ada@example.test?subject=Lunch&from=spoof@evil.test".to_owned(),
            subject: String::new(),
        });

        assert_eq!(prefill.to, "ada@example.test");
        assert_eq!(prefill.subject, "Lunch");
    }

    #[test]
    fn a_refused_file_crosses_with_its_reason() {
        let prefill = prefill_from_share(ShareRequest {
            files: vec![file("", "ghost.pdf", "application/pdf")],
            text: String::new(),
            subject: String::new(),
        });

        assert!(prefill.attachments.is_empty());
        assert!(prefill.is_empty, "nothing to open a composer with");
        assert_eq!(prefill.rejected.len(), 1);
        assert_eq!(prefill.rejected[0].name, "ghost.pdf");
        assert!(matches!(
            prefill.rejected[0].reason,
            ShareRejectionReason::NoPath
        ));
    }

    #[test]
    fn an_empty_share_is_answered_rather_than_refused() {
        let prefill = prefill_from_share(ShareRequest {
            files: Vec::new(),
            text: String::new(),
            subject: String::new(),
        });

        assert!(prefill.is_empty);
        assert!(prefill.rejected.is_empty());
    }
}
