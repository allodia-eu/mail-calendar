//! The FFI half of keeping a composed message on the server: the three verbs a composer
//! drives, the hint it renders, and the interval its idle timer counts.
//!
//! Its own file rather than an addition to [`crate::composer`], which is at the 500-line
//! limit. The rules these expose are in `docs/drafts.md`.

use mailcal_app::{
    CompositionId, DraftStatus as AppDraftStatus, DraftsIntent, Intent as AppIntent,
};

use crate::{
    MailcalApp, MailcalError,
    composer::{ComposerBlob, Recipients, prepare_rich, send_account},
};

/// The state of the most recent draft save (pulled after a `Surface::DraftStatus` signal).
///
/// **A hint, never a gate.** No state here should stop a composer being closed, and none is
/// worth a modal: saving a draft is not something the user asked about out loud.
#[derive(uniffi::Enum)]
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
    /// **A host calls this whenever a composer closes**, including after sending. Without it
    /// the core holds a record per composer for the life of the process, and a host that
    /// reuses a composition id would have its next draft supersede the previous one.
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
}

/// Names a composition, refusing a blank id.
///
/// Refused rather than defaulted: a blank id would merge every composer's draft into one
/// record, and each save would then supersede a different composer's stored copy.
fn composition_id(value: String) -> Result<CompositionId, MailcalError> {
    CompositionId::new(value)
        .ok_or_else(|| MailcalError::Engine("composition id is blank".to_owned()))
}
