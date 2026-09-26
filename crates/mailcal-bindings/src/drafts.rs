//! The FFI half of keeping a composed message on the server: the three verbs a composer
//! drives, the hint it renders, and the interval its idle timer counts.
//!
//! Its own file rather than an addition to [`crate::composer`], which is at the 500-line
//! limit. The rules these expose are in `docs/drafts.md`.

use std::sync::Arc;

use mailcal_app::{
    CompositionId, DraftStatus as AppDraftStatus, DraftsIntent, Intent as AppIntent,
};

use crate::{
    MailcalApp, MailcalError,
    composer::{ComposerBlob, Recipients, message_ref, prepare_rich, send_account},
    composer_files::{ComposerFileAttachment, prepare_with_files},
};

/// The state of the most recent draft save (pulled after a `Surface::DraftStatus` signal).
///
/// **A hint, never a gate.** No state here should stop a composer being closed, and none is
/// worth a modal: saving a draft is not something the user asked about out loud.
// Copy, because the one client that consumes these bindings as Rust keeps a status per open
// composer and reads it out of a map. It changes nothing for the generated languages.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    /// Nothing has been saved this session.
    Idle,
    /// A save is in flight.
    Saving,
    /// The draft is on the server. Also the answer to a save whose content was unchanged,
    /// which writes nothing and is still honestly saved.
    Saved,
    /// The save has **not** reached the server yet and is waiting for a network. Show it as
    /// pending, never as a failure: the words are not lost, and the save goes out by itself.
    Queued,
    /// The save did not reach the server and nothing will retry it.
    Failed,
}

impl From<AppDraftStatus> for DraftStatus {
    fn from(status: AppDraftStatus) -> Self {
        match status {
            AppDraftStatus::Idle => Self::Idle,
            AppDraftStatus::Saving => Self::Saving,
            AppDraftStatus::Saved => Self::Saved,
            AppDraftStatus::Queued => Self::Queued,
            AppDraftStatus::Failed => Self::Failed,
        }
    }
}

/// How many seconds a composer sits untouched before it saves.
///
/// Read rather than hard-coded, so the four clients cannot disagree about it. A client
/// restarts its timer on every keystroke: the trigger is the pause, not the clock.
#[must_use]
#[uniffi::export]
pub fn draft_autosave_idle_seconds() -> u64 {
    mailcal_app::DRAFT_AUTOSAVE_IDLE.as_secs()
}

#[uniffi::export]
impl MailcalApp {
    /// Stores the composer's content in the account's Drafts folder, replacing what this
    /// composition's previous save left there.
    ///
    /// `composition` names the composer, and the host keeps one id per open composer for as
    /// long as it lives: that is what lets two composers save without superseding each
    /// other's draft. `document_json` and `blobs` carry the body exactly as
    /// [`MailcalApp::submit_rich_mail`] does.
    ///
    /// Both the explicit "Save as draft" and the idle timer call this, and the core cannot
    /// tell them apart. Fire-and-forget: it returns once the document validates, and the
    /// observer fires on `Surface::DraftStatus` when the save settles. A save with no network
    /// is queued rather than lost, which the status reports as `Queued`.
    ///
    /// Unlike a send, empty recipients are fine: a draft is unfinished by definition.
    ///
    /// **A composer holding picked or resumed files calls
    /// [`MailcalApp::save_draft_with_files`] instead**, which reads their bytes; this one
    /// carries only what the editor itself holds.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Composer`] if `document_json` is invalid, validation fails, a
    /// blob handle is blank or a required blob is missing, and [`MailcalError::Engine`] if
    /// `composition` is blank or `from` is not an account id. A provider failure is reported
    /// through the status, not returned here.
    #[uniffi::method(default(from = None))]
    pub fn save_draft(
        &self,
        composition: String,
        recipients: Recipients,
        subject: String,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let composition = composition_id(composition)?;
        let (document, blobs) = prepare_rich(&document_json, blobs)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_dispatch(AppIntent::Drafts(DraftsIntent::Save {
            composition,
            from: send_account(from)?,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
        }));
        Ok(())
    }

    /// [`MailcalApp::save_draft`] for a composer holding **files**: the attachments the user
    /// picked, and the ones a resumed draft opened with.
    ///
    /// The byte read happens here rather than in the host, exactly as on
    /// [`MailcalApp::submit_rich_mail_with_files`], so file content does not cross the FFI.
    ///
    /// **A composer holding files saves through this one, never the other.** A save replaces
    /// the copy on the server, so a save that left the files out would take them off it: the
    /// user's attachment would be gone from a draft they are still writing, with nothing
    /// said.
    ///
    /// # Errors
    ///
    /// As [`MailcalApp::save_draft`], plus [`MailcalError::Composer`] when a selected file
    /// cannot be read.
    #[uniffi::method(default(from = None))]
    pub fn save_draft_with_files(
        &self,
        composition: String,
        recipients: Recipients,
        subject: String,
        document_json: String,
        files: Vec<ComposerFileAttachment>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let composition = composition_id(composition)?;
        let prepared = prepare_with_files(&document_json, files)?;
        let from = send_account(from)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_with_files(prepared, move |document, blobs| {
            AppIntent::Drafts(DraftsIntent::Save {
                composition,
                from,
                to,
                cc,
                bcc,
                subject,
                document,
                blobs,
            })
        });
        Ok(())
    }

    /// Removes this composition's stored draft from the server and forgets the composition.
    ///
    /// A composition that was never saved reaches no server: there is nothing there to
    /// remove, and calling this on one is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn discard_draft(&self, composition: String) -> Result<(), MailcalError> {
        self.spawn_dispatch(AppIntent::Drafts(DraftsIntent::Discard {
            composition: composition_id(composition)?,
        }));
        Ok(())
    }

    /// Forgets the composition, leaving the stored draft where it is: the composer closed and
    /// the draft stays in Drafts.
    ///
    /// **A host calls this when a composer closes without sending.** Without it the core holds
    /// a record per composer for the life of the process, and a host that reuses a composition
    /// id would have its next draft supersede the previous one.
    ///
    /// **Never after a submit that was accepted.** The send owns the composition from then on
    /// and finishes with it when the message settles, taking the stored draft away or leaving
    /// it. A host that closed it as well would be racing that: the two are separate tasks, and
    /// this one landing first leaves the send with no record to find and the draft in the
    /// user's Drafts folder for ever.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn close_composition(&self, composition: String) -> Result<(), MailcalError> {
        self.spawn_dispatch(AppIntent::Drafts(DraftsIntent::Close {
            composition: composition_id(composition)?,
        }));
        Ok(())
    }

    /// How `composition`'s most recent save ended.
    ///
    /// A `Surface::DraftStatus` signal says that *some* composition's save moved, not which,
    /// so every open composer re-pulls its own. One that has saved nothing yet, and one that
    /// has closed, both read `Idle`: never another composer's state.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank.
    pub fn draft_status(&self, composition: String) -> Result<DraftStatus, MailcalError> {
        Ok(self.app.draft_status(&composition_id(composition)?).into())
    }

    /// Opens the stored draft `key` (in `account`) into `composition`, so the composer the
    /// host is about to show saves over that copy rather than beside it.
    ///
    /// `composition` is the host's own, minted for that composer exactly as for a reply, and
    /// used from then on by `save_draft`, `discard_draft` and `close_composition`.
    ///
    /// **Open the composer with what this returns, not before it.** The draft's files are
    /// staged into `staging_directory` before it answers, and the composer must hold them:
    /// its next save replaces the stored copy, so a file it opened without is a file that
    /// save takes out of the user's mailbox. `staging_directory` is the host's own private
    /// area and the host owns what is left in it, as for a forward.
    ///
    /// **Call this off the UI thread.** It blocks on the internal runtime, and unlike
    /// staging a forward's files it cannot count on a warm cache: a draft is opened from a
    /// list row rather than from the reading view, so the first open of one fetches the
    /// message from the server.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] if `composition` is blank, the `account`/`key`
    /// reference is malformed, the draft cannot be resolved, **the message is not a draft**,
    /// its body cannot be read, or a file cannot be staged. Nothing is adopted on an error:
    /// show it rather than opening an empty composer, which would save over the draft.
    pub fn resume_draft(
        &self,
        composition: String,
        account: String,
        key: String,
        staging_directory: String,
    ) -> Result<DraftResume, MailcalError> {
        let composition = composition_id(composition)?;
        let message = message_ref(&account, key)?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move {
                app.resume_draft(composition, message, &staging_directory)
                    .await
            })
            .map(DraftResume::from)
            .map_err(MailcalError::Engine)
    }
}

/// A stored draft, as the composer resuming it opens.
#[derive(uniffi::Record)]
pub struct DraftResume {
    /// The account whose Drafts folder held it, and which its saves go back to. Show it in
    /// the From field: a later change there does not move the stored copy.
    pub account: String,
    /// The `To` field, comma-separated.
    pub to: String,
    /// The `Cc` field, comma-separated.
    pub cc: String,
    /// The `Bcc` field, comma-separated; usually empty for a draft saved elsewhere, since
    /// most transports do not hand a `Bcc` back.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text. Seed the editor with it the way a `mailto:` body is seeded.
    pub body_text: String,
    /// The files the draft carries, already written into the staging directory and ready to
    /// be attached exactly as a picked file is.
    pub attachments: Vec<ComposerFileAttachment>,
}

impl From<mailcal_app::DraftResume> for DraftResume {
    fn from(resumed: mailcal_app::DraftResume) -> Self {
        Self {
            account: resumed.account,
            to: resumed.to,
            cc: resumed.cc,
            bcc: resumed.bcc,
            subject: resumed.subject,
            body_text: resumed.body_text,
            attachments: resumed
                .attachments
                .into_iter()
                .map(|file| ComposerFileAttachment {
                    path: file.path,
                    file_name: file.file_name,
                    media_type: file.media_type,
                })
                .collect(),
        }
    }
}

/// Names the composition a message being **sent** was written in, when the host names one.
///
/// `None` (the default) means a send with no composer behind it, such as an assistant's. A
/// blank id is refused rather than dropped to `None`, on the same terms as a malformed
/// From-account ([`send_account`](crate::composer::send_account)): a send that quietly
/// forgot its composition leaves the draft in the user's Drafts folder after the message
/// has gone.
pub(crate) fn sent_composition(
    composition: Option<String>,
) -> Result<Option<CompositionId>, MailcalError> {
    composition.map(composition_id).transpose()
}

/// Names a composition, refusing a blank id.
///
/// Refused rather than defaulted: a blank id would merge every composer's draft into one
/// record, and each save would then supersede a different composer's stored copy.
fn composition_id(value: String) -> Result<CompositionId, MailcalError> {
    CompositionId::new(value)
        .ok_or_else(|| MailcalError::Engine("composition id is blank".to_owned()))
}
