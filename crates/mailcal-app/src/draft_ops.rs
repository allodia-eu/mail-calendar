//! Keeping the message being composed on the server: saving one, discarding one, and the
//! per-composition record that joins the two.
//!
//! The rules are in `docs/drafts.md`; two of them decide almost everything here. The stored
//! copy is named by the **key the provider answered with**, because three of the four adapters
//! move it on a re-save and the fourth moves the `Message-ID` instead. And the save is the one
//! mail write this app repeats on purpose, which is why every save of one composition carries
//! the same `Message-ID`: that is what the engine keys the queued op's resource on, so a fresh
//! one per save would leave each save queued *beside* the last rather than superseding it.
//!
//! **Privacy.** A draft is the user's mail. Nothing here logs its content; counts, ids and
//! classes only (`docs/logging.md`).

use core::time::Duration;
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use engine_api::{
    AccountId, Draft, DraftPut, DrainOutcome, DrainReport, EmailAddress, MessageIdHeader,
    PendingOpId, PendingOpKind, Provider, ProviderKey,
};
use mailcal_composer::ComposerDocument;

use crate::{
    App, ComposerBlob, CompositionId, DraftStatus, DraftsIntent, Surface,
    helpers::new_message_id,
    mail_compose::{parse_addresses, rich_draft},
};

/// How long a composer sits untouched before its draft is saved.
///
/// The core's constant rather than each client's, so the four cannot disagree about it. A
/// client restarts the interval on every keystroke: the trigger is the pause, not the clock,
/// so someone still typing is never made to wait on an upload (`docs/drafts.md`).
pub const DRAFT_AUTOSAVE_IDLE: Duration = Duration::from_secs(30);

/// What the core knows about one composition, for as long as its composer is open.
struct Composition {
    /// The account whose Drafts folder holds it. Fixed by the first save: the stored copy
    /// lives in one account's folder, and moving it later would leave that copy behind.
    account: AccountId,
    /// The header every save of this composition carries. See the module docs for why it is
    /// minted once rather than per save.
    message_id: MessageIdHeader,
    /// What the server stores the draft under **now**, or `None` until a save has actually
    /// reached one. Handed to the next save as `replacing`.
    key: Option<ProviderKey>,
    /// A digest of what the last save put on the server, so an unchanged draft costs no
    /// write. Compared only within this process, which is all
    /// [`DefaultHasher`] promises.
    saved: Option<u64>,
    /// The queued save waiting for a network, if the last one did not get through.
    ///
    /// Held so the drain that eventually stores it can hand its key back here. Nothing else
    /// can: the op settles with nobody watching, and a settled op is not in the queue read.
    /// Without it a composer that stayed open through the outage would save again with
    /// `replacing` empty and leave a second draft on the server.
    queued: Option<PendingOpId>,
}

/// The open compositions, keyed by the id their host minted.
#[derive(Default)]
pub(crate) struct DraftState {
    open: HashMap<CompositionId, Composition>,
}

impl<P: Provider> App<P> {
    /// Routes one [`DraftsIntent`] to its handler.
    pub(crate) async fn dispatch_drafts(&self, intent: DraftsIntent) {
        match intent {
            DraftsIntent::Save {
                composition,
                from,
                to,
                cc,
                bcc,
                subject,
                document,
                blobs,
            } => {
                self.save_draft(composition, from, to, cc, bcc, subject, document, blobs)
                    .await;
            }
            DraftsIntent::Discard { composition } => self.discard_draft(&composition).await,
            DraftsIntent::Close { composition } => self.close_composition(&composition),
        }
    }

    /// The most recent save's state, for the composer's quiet hint.
    #[must_use]
    pub fn draft_status(&self) -> DraftStatus {
        *self
            .draft_status
            .lock()
            .expect("draft-status mutex poisoned")
    }

    /// Stores the composer's content in the account's Drafts folder, replacing what this
    /// composition's previous save left there.
    #[allow(clippy::too_many_arguments)]
    async fn save_draft(
        &self,
        composition: CompositionId,
        from: Option<AccountId>,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        document: ComposerDocument,
        blobs: Vec<ComposerBlob>,
    ) {
        // An account already carrying this draft wins over the composer's dropdown: see
        // `Composition::account`.
        let account = match self.composition_account(&composition) {
            Some(account) => account,
            None => match from.or(self.compose_account().await) {
                Some(account) => account,
                None => return self.fail_draft("no account to save a draft to"),
            },
        };
        let Some(identity) = self.account_identity(&account).await else {
            return self.fail_draft("the account to save to is not configured");
        };
        let message_id = self.composition_message_id(&composition, &account);
        let Some(message_id) = message_id else {
            return self.fail_draft("could not mint a Message-ID for the draft");
        };
        let Some(draft) = build(
            message_id, &identity, &to, &cc, &bcc, subject, document, blobs,
        ) else {
            return self.fail_draft("the draft could not be rendered");
        };

        // Nothing changed since the last save, so there is nothing to write. Still "saved":
        // what the user asked to keep is on the server (`docs/drafts.md`).
        let digest = digest(&draft);
        if self.already_saved(&composition, digest) {
            return self.set_draft_status(DraftStatus::Saved);
        }

        self.set_draft_status(DraftStatus::Saving);
        let replacing = self.composition_key(&composition);
        match self.put_draft(&account, &draft, replacing.as_ref()).await {
            Some(Ok(key)) => {
                self.record_save(&composition, key, digest);
                self.set_draft_status(DraftStatus::Saved);
                // So the Drafts folder shows what was just put in it.
                self.refresh_after_write(&account).await;
            }
            // The engine parks a retryable failure and settles the rest, so the queue itself
            // answers "are these words lost?": asked of the store rather than re-derived from
            // the error here, which would be a second copy of a rule the engine owns. The
            // key is deliberately left alone; the drain records its own.
            Some(Err(err)) => {
                if let Some(op) = self.queued_save(&account, &draft).await {
                    log::info!("drafts: the save is queued until there is a network: {err}");
                    self.record_queued(&composition, op);
                    self.set_draft_status(DraftStatus::Queued);
                } else {
                    log::warn!("drafts: the save failed: {err}");
                    self.set_draft_status(DraftStatus::Failed);
                }
            }
            None => self.fail_draft("the account has no provider to save through"),
        }
    }

    /// Removes this composition's stored draft and forgets the composition.
    async fn discard_draft(&self, composition: &CompositionId) {
        let Some((account, key)) = self.forget(composition) else {
            // Either an unknown composition or one never saved. Nothing is on the server,
            // so discarding it is already done.
            return;
        };
        let Some(key) = key else { return };
        let Some(acct) = self.account_handle(&account).await else {
            return;
        };
        let Some(provider) = acct.providers.first() else {
            return;
        };
        match self.engine.delete_draft(provider, &account, &key).await {
            Ok(_) => self.refresh_after_write(&account).await,
            // Queued, like any other write the outbox holds: the copy goes when the network
            // does. Nothing to tell the composer, which has closed.
            Err(err) => log::info!("drafts: the discard did not reach the server yet: {err}"),
        }
    }

    /// Forgets the composition, leaving the stored draft where it is.
    fn close_composition(&self, composition: &CompositionId) {
        let _ = self.forget(composition);
    }

    /// Saves through `account`'s first provider. `None` when it has none to save through;
    /// the inner result is the engine's.
    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
    ) -> Option<Result<ProviderKey, engine_api::ApiError>> {
        let acct = self.account_handle(account).await?;
        let provider = acct.providers.first()?;
        Some(
            self.engine
                .put_draft(provider, account, draft, replacing)
                .await
                .map(|outcome| outcome.key),
        )
    }

    /// The queued save of `draft` sitting in `account`'s outbox, if the failure parked one.
    ///
    /// Matched on the `Message-ID`, which every save of one composition shares, so this finds
    /// the op whatever text it carries.
    async fn queued_save(&self, account: &AccountId, draft: &Draft) -> Option<PendingOpId> {
        let queued = self.engine.outbox(account).await.ok()?;
        queued
            .iter()
            .filter(|row| row.kind == Some(PendingOpKind::MailDraftPut))
            .find(|row| {
                serde_json::from_value::<DraftPut>(row.payload.clone())
                    .is_ok_and(|put| put.draft.message_id == draft.message_id)
            })
            .map(|row| row.id)
    }

    /// Takes the keys a drain pass resolved for queued saves, so the composer that is still
    /// open supersedes its own stored copy on the next save rather than storing a second one.
    ///
    /// Matched by op id, because that is all a [`DrainedOp`](engine_api::DrainedOp) carries
    /// that names *which* write it was.
    pub(crate) fn record_drained_drafts(&self, report: &DrainReport) {
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        for drained in &report.attempted {
            if drained.kind != PendingOpKind::MailDraftPut
                || drained.outcome != DrainOutcome::Succeeded
            {
                continue;
            }
            let Some(open) = state
                .open
                .values_mut()
                .find(|open| open.queued == Some(drained.id))
            else {
                continue;
            };
            open.queued = None;
            if let Some(key) = drained.provider_key.clone() {
                open.key = Some(key);
            }
        }
    }

    /// The account a composition is already stored under, if it has been saved.
    fn composition_account(&self, composition: &CompositionId) -> Option<AccountId> {
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get(composition)
            .map(|open| open.account.clone())
    }

    /// The header every save of this composition carries, starting one if this is its first
    /// save.
    fn composition_message_id(
        &self,
        composition: &CompositionId,
        account: &AccountId,
    ) -> Option<MessageIdHeader> {
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        if let Some(open) = state.open.get(composition) {
            return Some(open.message_id.clone());
        }
        let message_id = new_message_id()?;
        state.open.insert(
            composition.clone(),
            Composition {
                account: account.clone(),
                message_id: message_id.clone(),
                key: None,
                saved: None,
                queued: None,
            },
        );
        Some(message_id)
    }

    /// What the server stores this composition under now.
    fn composition_key(&self, composition: &CompositionId) -> Option<ProviderKey> {
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get(composition)
            .and_then(|open| open.key.clone())
    }

    /// Whether the last save put exactly this content on the server.
    fn already_saved(&self, composition: &CompositionId, digest: u64) -> bool {
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get(composition)
            .is_some_and(|open| open.saved == Some(digest))
    }

    /// Records what the save stored, and under which key.
    fn record_save(&self, composition: &CompositionId, key: ProviderKey, digest: u64) {
        if let Some(open) = self
            .drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get_mut(composition)
        {
            open.key = Some(key);
            open.saved = Some(digest);
            open.queued = None;
        }
    }

    /// Records the op a failed save left queued.
    fn record_queued(&self, composition: &CompositionId, op: PendingOpId) {
        if let Some(open) = self
            .drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .get_mut(composition)
        {
            open.queued = Some(op);
        }
    }

    /// Drops the composition, answering with the account and stored key it had.
    fn forget(&self, composition: &CompositionId) -> Option<(AccountId, Option<ProviderKey>)> {
        self.drafts
            .lock()
            .expect("drafts mutex poisoned")
            .open
            .remove(composition)
            .map(|open| (open.account, open.key))
    }

    /// Reports a save that never reached a provider call.
    fn fail_draft(&self, reason: &str) {
        log::warn!("drafts: {reason}");
        self.set_draft_status(DraftStatus::Failed);
    }

    /// Publishes the draft-save hint.
    ///
    /// Unlike the send hint this does **not** auto-clear. A composer stays open across many
    /// saves, and "saved" is the standing truth about the draft in it until the next save
    /// changes it; blanking it after a couple of seconds would leave the composer saying
    /// nothing about a draft that is safely on the server.
    fn set_draft_status(&self, status: DraftStatus) {
        *self
            .draft_status
            .lock()
            .expect("draft-status mutex poisoned") = status;
        self.observer.surface_changed(Surface::DraftStatus);
    }
}

/// Renders the composer's content into the draft to store.
///
/// The same builder a send uses, so the quoted original and the signature pass the same
/// sanitisation gate on the way to the server (`docs/composer-security.md`). Unlike a send,
/// an empty recipient list is fine: a draft is unfinished by definition.
#[allow(clippy::too_many_arguments)]
fn build(
    message_id: MessageIdHeader,
    identity: &EmailAddress,
    to: &str,
    cc: &str,
    bcc: &str,
    subject: String,
    document: ComposerDocument,
    blobs: Vec<ComposerBlob>,
) -> Option<Draft> {
    rich_draft(
        message_id,
        identity,
        parse_addresses(to),
        parse_addresses(cc),
        parse_addresses(bcc),
        subject,
        document,
        blobs,
        &[],
    )
}

/// A digest of everything a save would put on the server.
///
/// Over the serialized draft rather than a chosen subset of its fields: a subset is a list
/// that goes stale the moment the draft grows a field, and the failure is silent, an edit the
/// user made that never leaves the device.
fn digest(draft: &Draft) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_vec(draft)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}
