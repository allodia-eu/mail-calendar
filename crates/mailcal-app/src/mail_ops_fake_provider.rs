//! The submitting provider fake the send tests drive: it records what it was asked to submit,
//! and can be told to fail the send, to deliver without filing the sender's copy, or to fail
//! the after-the-fact repair as well.
//!
//! Split from [`super`] (the tests themselves) so each file stays under the 500-line limit.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Draft, ProviderKey, SubmissionReceipt};
use engine_provider::{Capabilities, ConnectionInfo, Provider, ProviderError, ProviderResult};
use tokio::sync::Notify;

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
        }
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
        let key = ProviderKey::new("sent-1").unwrap();
        let id = draft.message_id.clone();
        if self.unfiled {
            let detail = "IMAP transport error: connection reset by peer";
            return Ok(SubmissionReceipt::unfiled(key, id, detail));
        }
        Ok(SubmissionReceipt::filed(key, id))
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
