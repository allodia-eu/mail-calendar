//! Opening the composer for a share: files the desktop handed the app to send
//! (`docs/os-integration.md`).
//!
//! The twin of the mail-link path in [`super::composer_prefill`], and it behaves identically where
//! the two meet: a share arriving before the first account is kept until one exists, and a share
//! arriving over a draft asks before replacing it, because a launch the user did not aim at this
//! window must never throw away what they were writing.
//!
//! What is different is the payload: the composer opens holding attachments the core did not stage,
//! laid on top of the ones it did. Their names and media types were decided by the shared core, so
//! nothing here inspects a file.
//!
//! One consequence is worth knowing before it surprises someone: a share-opened composer counts as
//! **dirty from the moment it opens**, because [`super::composer_draft::headers_edited`] measures
//! the file count against the files the core staged, and a share's are not among them. So
//! navigating away from one asks, where navigating away from an untouched reply does not. That is
//! the rule working, not a leak: the file was the user's choice in their file manager, exactly as a
//! picked one is their choice in the dialog. The files the core staged, a forward's originals or a
//! stored draft's own, read the other way for the same reason: they are still in the mailbox, so
//! abandoning the composer loses nothing.

use mailcal_bindings::{ComposeOrigin, ComposePrefill, SharePrefill};

use super::{AppInput, AppModel, composer_model::PickedFile, composer_open::ComposerTarget};

impl AppModel {
    /// Opens a composer for what another application shared with this one: a new message from
    /// the folder's account, holding the shared files on top of whatever the core opened it with.
    ///
    /// The recipient fields come straight through: they are non-empty only when the shared text
    /// was itself a `mailto:` link, which the core decoded through the mail-link allowlist. A
    /// sharing app has no other way to address a message.
    ///
    /// A share carrying nothing usable is ignored rather than answered with a blank composer: the
    /// user asked to send *those files*, and an empty message over whatever they were reading
    /// would be a worse answer than none.
    pub(super) fn open_share(&mut self, prefill: SharePrefill, sender: &relm4::Sender<AppInput>) {
        if prefill.is_empty {
            log::info!("share carried nothing to open a composer with");
            return;
        }
        if self.snapshot.accounts.is_empty() {
            self.pending_share = Some(prefill);
            self.primary = super::PrimaryView::Mail;
            return;
        }
        let (prefill, files) = share_prefill(prefill);
        self.open_composer_on(
            ComposeOrigin::New {
                account: self.snapshot.selected_account.clone(),
            },
            Some(prefill),
            ComposerTarget::Pane,
            files,
            sender,
        );
    }

    /// Opens a share that arrived before there was an account to send it from, now that there is
    /// one. Called from the same place the held mail link is.
    pub(super) fn try_open_pending_share(&mut self, sender: &relm4::Sender<AppInput>) {
        if !self.snapshot.accounts.is_empty()
            && let Some(prefill) = self.pending_share.take()
        {
            self.open_share(prefill, sender);
        }
    }
}

/// A share's fields, laid over a new message, and the files it brings. Its text brings no
/// sign-off of its own.
fn share_prefill(prefill: SharePrefill) -> (ComposePrefill, Vec<PickedFile>) {
    let files = prefill
        .attachments
        .into_iter()
        .map(|attachment| PickedFile {
            path: attachment.path,
            file_name: attachment.file_name,
            media_type: attachment.media_type,
        })
        .collect();
    let fields = ComposePrefill {
        to: prefill.to,
        cc: prefill.cc,
        bcc: prefill.bcc,
        subject: prefill.subject,
        body_text: prefill.body,
        signed: false,
        account: None,
    };
    (fields, files)
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{ComposerFileAttachment, SharePrefill};

    use super::share_prefill;

    #[test]
    fn a_share_fills_a_new_message_and_brings_its_files_alongside() {
        let (fields, files) = share_prefill(SharePrefill {
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            subject: "Holiday".to_owned(),
            body: "Pictures attached".to_owned(),
            attachments: vec![ComposerFileAttachment {
                path: "/shared/beach.jpg".to_owned(),
                file_name: "beach.jpg".to_owned(),
                media_type: "image/jpeg".to_owned(),
            }],
            rejected: Vec::new(),
            is_empty: false,
        });
        assert_eq!(fields.subject, "Holiday");
        assert_eq!(fields.body_text, "Pictures attached");
        assert!(!fields.signed, "shared text brings no sign-off of its own");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "/shared/beach.jpg");
    }
}
