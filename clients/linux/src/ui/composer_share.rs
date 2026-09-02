//! Opening the composer for a share: files the desktop handed the app to send
//! (`docs/os-integration.md`).
//!
//! The twin of the mail-link path in [`super::composer_draft`], and it behaves identically where
//! the two meet: a share arriving before the first account is kept until one exists, and a share
//! arriving over a draft asks before replacing it, because a launch the user did not aim at this
//! window must never throw away what they were writing.
//!
//! What is different is the payload: the composer opens already holding attachments, which no
//! other route does. Their names and media types were decided by the shared core, so nothing here
//! inspects a file.
//!
//! One consequence is worth knowing before it surprises someone: a share-opened composer counts as
//! **dirty from the moment it opens**, because [`super::composer_draft::headers_edited`] treats any
//! attachment as something to lose. So navigating away from one asks, where navigating away from an
//! untouched reply does not. That is the rule working, not a leak: the file was the user's choice
//! in their file manager, exactly as a picked one is their choice in the dialog.

use mailcal_bindings::SharePrefill;

use super::{
    AppModel, PendingNavigation,
    composer_model::{ComposeKind, ComposeRequest, PickedFile},
};

impl ComposeRequest {
    /// A new message pre-filled from a share.
    ///
    /// The recipient fields come straight through: they are non-empty only when the shared text
    /// was itself a `mailto:` link, which the core decoded through the mail-link allowlist. A
    /// sharing app has no other way to address a message.
    pub(crate) fn from_share(prefill: SharePrefill, initial_from: Option<String>) -> Self {
        Self {
            kind: ComposeKind::New,
            account: None,
            key: None,
            initial_to: prefill.to,
            initial_cc: prefill.cc,
            initial_bcc: prefill.bcc,
            subject: prefill.subject,
            initial_body: (!prefill.body.is_empty()).then_some(prefill.body),
            quote: None,
            initial_from,
            seeds_signature: true,
            files: prefill
                .attachments
                .into_iter()
                .map(|attachment| PickedFile {
                    path: attachment.path,
                    file_name: attachment.file_name,
                    media_type: attachment.media_type,
                })
                .collect(),
        }
    }
}

impl AppModel {
    /// Opens a composer for what another application shared with this one.
    ///
    /// A share carrying nothing usable is ignored rather than answered with a blank composer: the
    /// user asked to send *those files*, and an empty message over whatever they were reading
    /// would be a worse answer than none.
    pub(super) fn open_share(&mut self, prefill: SharePrefill) {
        if prefill.is_empty {
            log::info!("share carried nothing to open a composer with");
            return;
        }
        if self.snapshot.accounts.is_empty() {
            self.pending_share = Some(prefill);
            self.primary = super::PrimaryView::Mail;
            return;
        }
        let initial_from = self.app.as_ref().and_then(|app| {
            super::composer_model::initial_sender(
                None,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            )
        });
        let request = ComposeRequest::from_share(prefill, initial_from);
        if self.composer.is_some() {
            self.queue_navigation(PendingNavigation::Composer(request));
        } else {
            self.commit_composer(request);
        }
    }

    /// Opens a share that arrived before there was an account to send it from, now that there is
    /// one. Called from the same place the held mail link is.
    pub(super) fn try_open_pending_share(&mut self) {
        if !self.snapshot.accounts.is_empty()
            && let Some(prefill) = self.pending_share.take()
        {
            self.open_share(prefill);
        }
    }
}
