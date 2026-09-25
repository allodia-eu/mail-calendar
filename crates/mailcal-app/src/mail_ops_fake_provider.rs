//! The submitting provider fake the send tests drive: it records what it was asked to submit,
//! and can be told to fail the send, to deliver without filing the sender's copy, or to fail
//! the after-the-fact repair as well. It stores drafts too, moving the key on every save the
//! way three of the four real adapters do.
//!
//! Split from [`super`] (the tests themselves) so each file stays under the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, CalendarWrites, Draft, ProviderKey, SubmissionReceipt};
use engine_provider::{Capabilities, ConnectionInfo, Provider, ProviderError, ProviderResult};
use tokio::sync::Notify;

/// One recorded draft save: what was stored, and the key it said it was superseding.
pub(super) type DraftSave = (Draft, Option<ProviderKey>);

pub(super) struct SubmitProvider {
    caps: Capabilities,
    submissions: Arc<Mutex<Vec<Draft>>>,
    /// When set, `submit_email` records the attempt then fails with a **permanent** provider error
    /// carrying this detail; used to drive the send-denied → mail-reconnect-prompt path (a Graph
    /// `403 ErrorAccessDenied` when the grant lacks `Mail.Send`).
    fail_detail: Option<String>,
    /// When set, `submit_email` **delivers** but reports the sender's copy as unfiled; an
    /// IMAP account whose Sent `APPEND` found a dead session.
    unfiled: bool,
    /// How many times the host asked for the copy to be filed after the fact, and whether
    /// those attempts succeed.
    refiles: Arc<Mutex<usize>>,
    refile_fails: bool,
    /// When set, `file_sent_copy` parks until it is notified: the seam a test needs to hold a
    /// repair open and do something else while it is in flight.
    repair_gate: Option<Arc<Notify>>,
    /// Sends left to refuse as **retryable** before this provider starts accepting them: no
    /// network, then a network. The one shape that queues rather than failing outright.
    offline_sends: Arc<Mutex<u32>>,
    /// Every draft this provider was asked to store, with the key each save said it was
    /// superseding. The pair, not just the draft: whether the caller names the copy it is
    /// replacing is the whole draft contract (`docs/drafts.md`), and a log of drafts alone
    /// cannot tell a correct save from one storing a second copy.
    draft_puts: Arc<Mutex<Vec<DraftSave>>>,
    /// Every stored draft this provider was asked to remove.
    draft_deletes: Arc<Mutex<Vec<ProviderKey>>>,
    /// Draft saves left to refuse as retryable, as `offline_sends` does for sends.
    offline_saves: Arc<Mutex<u32>>,
    /// How many saves succeed before the network goes, when it goes that way round. `None`
    /// leaves the provider on whatever `offline_saves` says.
    saves_before_outage: Arc<Mutex<Option<u32>>>,
    /// When set, `put_draft` records that it was entered and then parks until it is
    /// notified: the seam a test needs to hold one save open and start a second while it is
    /// in flight, which is the only way to observe whether the two overlap.
    save_gate: Option<Arc<Notify>>,
}

impl SubmitProvider {
    pub(super) fn new() -> Self {
        Self {
            caps: Capabilities::none().with_submission(),
            submissions: Arc::new(Mutex::new(Vec::new())),
            fail_detail: None,
            unfiled: false,
            refiles: Arc::new(Mutex::new(0)),
            refile_fails: false,
            repair_gate: None,
            offline_sends: Arc::new(Mutex::new(0)),
            draft_puts: Arc::new(Mutex::new(Vec::new())),
            draft_deletes: Arc::new(Mutex::new(Vec::new())),
            offline_saves: Arc::new(Mutex::new(0)),
            saves_before_outage: Arc::new(Mutex::new(None)),
            save_gate: None,
        }
    }

    /// A provider whose next `saves` draft saves fail retryably and which then works.
    pub(super) fn saving_nothing_for(saves: u32) -> Self {
        let provider = Self::new();
        *provider.offline_saves.lock().unwrap() = saves;
        provider
    }

    /// A provider whose first `saves` draft saves land and whose every save after that fails
    /// retryably: a composer that saved, then lost the network. The other way round from
    /// [`Self::saving_nothing_for`], and the only shape that can leave a composition holding
    /// both a recorded digest and a queued op.
    pub(super) fn saving_nothing_after(saves: u32) -> Self {
        let provider = Self::new();
        *provider.saves_before_outage.lock().unwrap() = Some(saves);
        provider
    }

    /// A provider whose draft saves park in `put_draft` until `gate` is notified, after
    /// recording that they got that far.
    pub(super) fn saving_until(gate: &Arc<Notify>) -> Self {
        Self {
            save_gate: Some(Arc::clone(gate)),
            ..Self::new()
        }
    }

    /// A provider with no network at all: every send fails retryably, so every message
    /// queues and nothing is ever delivered.
    pub(super) fn offline() -> Self {
        Self::offline_for(u32::MAX)
    }

    /// A provider whose next `sends` fail retryably and which then works: the outage a
    /// queued message is supposed to ride out.
    pub(super) fn offline_for(sends: u32) -> Self {
        let provider = Self::new();
        *provider.offline_sends.lock().unwrap() = sends;
        provider
    }

    /// A submitting provider whose every send fails with a permanent error carrying `detail`.
    pub(super) fn failing_with(detail: &str) -> Self {
        Self {
            fail_detail: Some(detail.to_owned()),
            ..Self::new()
        }
    }

    /// A provider whose sends go out but whose Sent copies never land.
    pub(super) fn filing_nothing() -> Self {
        Self {
            unfiled: true,
            ..Self::new()
        }
    }

    /// As [`Self::filing_nothing`], but the after-the-fact repair fails too.
    pub(super) fn filing_nothing_ever() -> Self {
        Self {
            refile_fails: true,
            ..Self::filing_nothing()
        }
    }

    /// A provider whose sends never file their copy and whose repair **parks** until the
    /// returned handle is notified, so a test can interleave a second failed send with a
    /// repair that is still in flight.
    pub(super) fn filing_nothing_until(gate: &Arc<Notify>) -> Self {
        Self {
            repair_gate: Some(Arc::clone(gate)),
            ..Self::filing_nothing()
        }
    }

    pub(super) fn refiles(&self) -> Arc<Mutex<usize>> {
        Arc::clone(&self.refiles)
    }

    pub(super) fn submissions(&self) -> Arc<Mutex<Vec<Draft>>> {
        Arc::clone(&self.submissions)
    }

    pub(super) fn draft_puts(&self) -> Arc<Mutex<Vec<DraftSave>>> {
        Arc::clone(&self.draft_puts)
    }

    pub(super) fn draft_deletes(&self) -> Arc<Mutex<Vec<ProviderKey>>> {
        Arc::clone(&self.draft_deletes)
    }
}

#[async_trait::async_trait]
impl Provider for SubmitProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }

    async fn submit_email(
        &self,
        _account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        self.submissions.lock().unwrap().push(draft.clone());
        if let Some(detail) = &self.fail_detail {
            return Err(ProviderError::permanent(detail.clone()));
        }
        {
            let mut offline = self.offline_sends.lock().unwrap();
            if *offline > 0 {
                *offline = offline.saturating_sub(1);
                return Err(ProviderError::retryable("no route to host"));
            }
        }
        let key = ProviderKey::new("sent-1").unwrap();
        let id = draft.message_id.clone();
        if self.unfiled {
            let detail = "IMAP transport error: connection reset by peer";
            return Ok(SubmissionReceipt::unfiled(key, id, detail));
        }
        Ok(SubmissionReceipt::filed(key, id))
    }

    /// Stores a draft, answering with a key that **moves on every save**, which is what
    /// three of the four real adapters do. A fake echoing the key back would let a caller
    /// that never passes `replacing` pass this suite and store a second copy per save
    /// against a real server.
    async fn put_draft(
        &self,
        _account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
    ) -> ProviderResult<ProviderKey> {
        {
            let mut offline = self.offline_saves.lock().unwrap();
            if *offline > 0 {
                *offline = offline.saturating_sub(1);
                return Err(ProviderError::retryable("no route to host"));
            }
        }
        {
            let mut remaining = self.saves_before_outage.lock().unwrap();
            if let Some(left) = remaining.as_mut() {
                if *left == 0 {
                    return Err(ProviderError::retryable("no route to host"));
                }
                *left -= 1;
            }
        }
        // Recorded on the way in, not on the way out, so the log counts saves that *entered*
        // the provider. That is what makes an overlap visible: two saves in flight at once
        // show as two rows while both are still parked on the gate.
        let key = {
            let mut puts = self.draft_puts.lock().unwrap();
            puts.push((draft.clone(), replacing.cloned()));
            ProviderKey::new(format!("draft-{}", puts.len())).unwrap()
        };
        if let Some(gate) = &self.save_gate {
            gate.notified().await;
        }
        Ok(key)
    }

    async fn delete_draft(&self, _account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.draft_deletes.lock().unwrap().push(draft.clone());
        Ok(())
    }

    async fn file_sent_copy(
        &self,
        _account: &AccountId,
        _draft: &Draft,
    ) -> ProviderResult<ProviderKey> {
        *self.refiles.lock().unwrap() += 1;
        if let Some(gate) = &self.repair_gate {
            gate.notified().await;
        }
        if self.refile_fails {
            return Err(ProviderError::retryable("still unreachable"));
        }
        Ok(ProviderKey::new("sent-1").unwrap())
    }
}

impl engine_api::MailboxWrites for SubmitProvider {}
impl CalendarWrites for SubmitProvider {}
