//! The JMAP-shaped [`FakeProvider`] test fixture and its builder variants. Split out of
//! `tests_fakes.rs` to keep each file under the size limit; a submodule of the shared
//! `fakes` module.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use engine_api::{AccountId, CalendarWrites};
use engine_core::{
    ids::MailboxId,
    mail::{Mailbox, MailboxRole, Message},
    raw::RawMime,
    sync::{JmapDataType, MailboxWindows, SyncScope, SyncState, SyncUpdate},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, MailEdit, MailEditReceipt, MailboxEdit,
    MailboxEditReceipt, MessageReport, Provider, ProviderError, ProviderResult, ReportReceipt,
    ScopeSync, SourceStream,
};
use futures::StreamExt;
use tokio::sync::Notify;

use super::message;

#[path = "provider_builders.rs"]
mod builders;
#[path = "provider_chunks.rs"]
mod chunks;
pub(crate) use builders::SourceFailure;
use chunks::{pages, reconcile_chunks};

/// A minimal JMAP-shaped fake: one Inbox and a configurable set of messages, a snapshot on
/// first sync of a scope and an empty delta afterwards. Records the edits it receives so a
/// test can assert the app routed the right [`MailEdit`] to the right account's provider.
pub(crate) struct FakeProvider {
    caps: Capabilities,
    mailboxes: Vec<Mailbox>,
    messages: Vec<Message>,
    edits: Arc<Mutex<Vec<MailEdit>>>,
    /// The reports this provider received, so a test can prove the app sent a **report**
    /// rather than a bare move: the two are indistinguishable from the row's point of view.
    reports: Arc<Mutex<Vec<MessageReport>>>,
    /// When set, the provider is bound to this one folder for email (a distinct
    /// per-mailbox scope), as the on-demand connector returns; `None` is the inbox
    /// provider's account-global email scope.
    email_mailbox: Option<MailboxId>,
    /// What [`Provider::connection_info`] reports as the transport's fetch width; how many
    /// single-object fetches a caller may keep in flight. `1` models a session protocol
    /// sharing one socket (IMAP); higher models an HTTP transport.
    concurrent_fetches: usize,
    /// What [`Provider::connection_info`] reports as the sources one batch request carries;
    /// `1` models a transport with no batch, higher models an IMAP `UID FETCH` over a set.
    sources_per_request: usize,
    /// The keys of every [`Provider::fetch_message_sources`] call, in order, so a test can
    /// prove the warm batches its fetches rather than asking one message at a time, and in
    /// which order it asks.
    batches: Arc<Mutex<Vec<Vec<String>>>>,
    /// The most [`Provider::fetch_message_source`] calls that were ever in flight at once,
    /// so a test can prove the body warm actually overlaps them rather than trickling.
    peak_in_flight: Arc<Mutex<(usize, usize)>>,
    /// When set, every [`Provider::fetch_message_source`] returns these raw RFC 5322
    /// bytes instead of the default hostile HTML body: so a reading test can drive a
    /// specific message shape (e.g. a `multipart/related` with an inline `cid:` image).
    source_override: Option<Vec<u8>>,
    /// When flipped on, every sync fails with a retryable transport error, models an
    /// account whose server can't be reached, so a test can assert it is badged unreachable.
    /// A shared flag ([`failure_switch`](Self::failure_switch)) so a test can toggle it
    /// between refreshes to exercise recovery.
    fail: Arc<AtomicBool>,
    /// When on, every edit is refused for good ([`refusal_switch`](Self::refusal_switch)).
    refuse_edits: Arc<AtomicBool>,
    /// How many times the app has asked this provider to stream email; i.e. how many syncs it
    /// has driven. Shared ([`syncs`](Self::syncs)) so a test can assert that a *burst* of writes
    /// coalesced into one account-wide re-sync rather than one per message.
    syncs: Arc<AtomicUsize>,
    /// Optional test gate that blocks the stream after the first committed chunk.
    stream_gate: Option<StreamGate>,
    /// Mail that arrives only on a **later** (cursored) sync: a reply landing after the first
    /// pass already threaded the mailbox. Shared ([`late_delivery`](Self::late_delivery)) so a
    /// test can post a message between two refreshes.
    late: Arc<Mutex<Vec<Message>>>,
    /// When set, every sync fails with an **authentication** class error: a server refusing the
    /// account's credential ([`refusing_signin`](Self::refusing_signin)), which the app classifies
    /// as an expired sign-in and not an outage.
    refuses_signin: bool,
    /// How many times this provider has been **asked** for a message's raw source;
    /// cumulative, where [`peak_in_flight`](Self::peak_in_flight) is a concurrency high-water
    /// mark. Shared ([`source_fetches`](Self::source_fetches)) so a test can prove a pass went
    /// back to the network for bytes rather than reading them from the cache.
    source_fetches: Arc<AtomicUsize>,
    /// Optional test gate that holds every [`Provider::edit_mail`] open until released, so a
    /// test can put a *second* write to one message inside the first's round trip: the shape a
    /// host produces by marking a message read when it opens and archiving it a moment later.
    edit_gate: Option<EditGate>,
    /// Message keys whose [`Provider::fetch_message_source`] fails, and how.
    source_failures: Vec<(String, SourceFailure)>,
    /// How many of the next source fetches the server refuses as throttled, and the wait each
    /// refusal names (`None`: a bare `429`, as Stalwart sends). Shared, so a test can read how
    /// many refusals are left.
    throttled_fetches: Arc<AtomicUsize>,
    throttle_wait_secs: Option<u64>,
    /// A folder tree the account can change, when set: every folder-list sync reads it whole
    /// and every folder change is applied to it, with ids that survive a rename (the JMAP
    /// shape). `None` is a provider without folder writes.
    tree: Option<Arc<Mutex<Vec<Mailbox>>>>,
    /// When set, a scope's first pass fetches only what its windows admit, as a real adapter's
    /// snapshot does, and each pass's windows are recorded here.
    windowed: Option<Arc<Mutex<Vec<MailboxWindows>>>>,
}

/// Decrements the in-flight count however a source fetch leaves.
struct InFlightGuard(Arc<Mutex<(usize, usize)>>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.lock().unwrap().0 -= 1;
    }
}

/// Holds each [`Provider::edit_mail`] until a test releases it, announcing its arrival first.
pub(crate) struct EditGate {
    /// Signalled as each edit enters, so a test waits for the write to be genuinely in
    /// flight rather than sleeping and hoping.
    entered: Arc<Notify>,
    /// Awaited by each edit; a test releases them by notifying.
    release: Arc<Notify>,
}

impl EditGate {
    pub(crate) fn new() -> Self {
        Self {
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }

    /// Waits until an edit has entered the provider.
    pub(crate) async fn await_entry(&self) {
        self.entered.notified().await;
    }

    /// Lets one waiting edit through.
    pub(crate) fn release_one(&self) {
        self.release.notify_one();
    }

    fn share(&self) -> Self {
        Self {
            entered: Arc::clone(&self.entered),
            release: Arc::clone(&self.release),
        }
    }
}

struct StreamGate {
    after_commit: Arc<Notify>,
    finish: Arc<Notify>,
}

#[async_trait::async_trait]
#[async_trait::async_trait]
impl Provider for FakeProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
            .with_concurrent_fetches(self.concurrent_fetches)
            .with_sources_per_request(self.sources_per_request)
    }

    // Each message still goes through `fetch_message_source`, so the in-flight counter and the
    // scripted failures apply to a batch exactly as to a single fetch.
    fn fetch_message_sources<'a>(
        &'a self,
        account: &'a AccountId,
        messages: &'a [Message],
    ) -> SourceStream<'a> {
        self.batches.lock().unwrap().push(
            messages
                .iter()
                .map(|message| message.id.key().as_str().to_owned())
                .collect(),
        );
        Box::pin(futures::stream::iter(messages.iter().enumerate()).then(
            move |(index, message)| async move {
                (index, self.fetch_message_source(account, message).await)
            },
        ))
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: JmapDataType::Mailbox,
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        match &self.email_mailbox {
            Some(mailbox) => SyncScope::ImapMailbox {
                account: account.clone(),
                mailbox: mailbox.clone(),
            },
            None => SyncScope::JmapType {
                account: account.clone(),
                data_type: JmapDataType::Email,
            },
        }
    }

    async fn sync_mailboxes(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        if self.refuses_signin {
            return Err(ProviderError::authentication("authentication failed"));
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("account unreachable"));
        }
        if let Some(tree) = &self.tree {
            let folders = tree.lock().unwrap().clone();
            let present = folders.iter().map(|m| m.id.key().clone()).collect();
            return Ok(ScopeSync::new(
                SyncUpdate::snapshot(folders, present),
                SyncState::new("tree"),
            ));
        }
        if cursor.is_some() {
            return Ok(ScopeSync::new(
                SyncUpdate::delta(Vec::new(), Vec::new()),
                SyncState::new("mbox-2"),
            ));
        }
        let present = self.mailboxes.iter().map(|m| m.id.key().clone()).collect();
        Ok(ScopeSync::new(
            SyncUpdate::snapshot(self.mailboxes.clone(), present),
            SyncState::new("mbox-1"),
        ))
    }

    fn stream_email<'a>(
        &'a self,
        _account: &AccountId,
        cursor: Option<&'a SyncState>,
        window: MailboxWindows,
        _fetch_batch: usize,
        chunk_size: usize,
    ) -> EmailStream<'a> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        let messages: Vec<Message> = match &self.windowed {
            Some(seen) => {
                seen.lock().unwrap().push(window.clone());
                let date = |m: &Message| m.received_at.or(m.sent_at);
                let admits = |m: &&Message| window.admits(m.mailboxes.iter(), date(m));
                self.messages.iter().filter(admits).cloned().collect()
            }
            None => self.messages.clone(),
        };
        if self.refuses_signin {
            return Box::pin(futures::stream::iter(vec![Err(
                ProviderError::authentication("authentication failed"),
            )]));
        }
        if self.fail.load(Ordering::SeqCst) {
            return Box::pin(futures::stream::iter(vec![Err(ProviderError::retryable(
                "account unreachable",
            ))]));
        }
        let chunks = if cursor.is_some() {
            // A later pass carries only what arrived since the cursor, nothing, unless a test
            // posted a late delivery. Additive, as a real cursored provider sync is: the mail
            // already in the store is neither re-sent nor reconciled away.
            let late = self.late.lock().unwrap().clone();
            let total = late.len();
            vec![EmailChunk::additive(
                late,
                Vec::new(),
                Some(total),
                SyncState::new("email-2"),
            )]
        } else if self.stream_gate.is_some() {
            pages(&messages, chunk_size)
                .map(|page| {
                    EmailChunk::additive(
                        page.to_vec(),
                        Vec::new(),
                        Some(messages.len()),
                        SyncState::new("email-1"),
                    )
                })
                .collect()
        } else {
            reconcile_chunks(&messages, chunk_size)
        };
        if let Some(gate) = &self.stream_gate {
            let after_commit = Arc::clone(&gate.after_commit);
            let finish = Arc::clone(&gate.finish);
            return Box::pin(async_stream::try_stream! {
                let mut rest = chunks.into_iter();
                if let Some(first) = rest.next() {
                    yield first;
                }
                after_commit.notify_one();
                finish.notified().await;
                for chunk in rest {
                    yield chunk;
                }
            });
        }
        Box::pin(futures::stream::iter(
            chunks.into_iter().map(Ok).collect::<Vec<_>>(),
        ))
    }

    async fn edit_mail(
        &self,
        _account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        // A downed provider refuses an edit for now; one refusing edits refuses it for good.
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("account unreachable"));
        }
        if self.refuse_edits.load(Ordering::SeqCst) {
            return Err(ProviderError::permanent("edit refused"));
        }
        if let Some(gate) = &self.edit_gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        self.edits.lock().unwrap().push(edit.clone());
        Ok(MailEditReceipt::new(edit.target().clone()))
    }

    async fn fetch_message_source(
        &self,
        _account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        // Record the concurrency before anything can return early, and yield once, so a
        // caller that overlaps its fetches is visible: without the yield every answer is ready
        // on its first poll and even a concurrent caller never holds two at once.
        // The guard is armed before the first await, so a cancelled fetch cannot leave the
        // count raised and inflate a later peak.
        self.source_fetches.fetch_add(1, Ordering::SeqCst);
        let _drop = {
            let mut seen = self.peak_in_flight.lock().unwrap();
            seen.0 += 1;
            seen.1 = seen.1.max(seen.0);
            InFlightGuard(Arc::clone(&self.peak_in_flight))
        };
        tokio::task::yield_now().await;
        // A downed provider can't fetch a source either; lets a test model an offline open and
        // prove a warmed body is served from the store's cache rather than the network.
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("account unreachable"));
        }
        let throttled = self
            .throttled_fetches
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if throttled {
            let wait = self.throttle_wait_secs.map(|secs| {
                engine_core::time::Duration::from_parts(0, 0, 0, 0, secs, 0).expect("in range")
            });
            return Err(ProviderError::rate_limited("too many requests", wait));
        }
        let key = message.id.key().as_str();
        if let Some((_, failure)) = self.source_failures.iter().find(|(k, _)| k == key) {
            return Err(failure.error());
        }
        if let Some(raw) = &self.source_override {
            return Ok(RawMime::new(raw.clone()));
        }
        // An HTML message carrying hostile content (a script + a remote tracking image)
        // so a test can prove the body is sanitised before the host sees it.
        Ok(RawMime::new(
            concat!(
                "Content-Type: text/html; charset=utf-8\r\n\r\n",
                "<html><body><p>See the <b>summary</b>.</p>",
                "<script>steal(document.cookie)</script>",
                "<img src=\"https://tracker.example/p.gif\"></body></html>",
            )
            .as_bytes()
            .to_vec(),
        ))
    }

    async fn report_message(
        &self,
        _account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        // A downed provider can't report either, so a test can prove a refused report
        // surfaces as `MailActionError::Rejected` rather than a silent success.
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("fake provider is down"));
        }
        // The same refusal a real adapter makes for a verdict its transport lacks; the
        // capability is what a caller must read, and this proves it did.
        self.caps
            .mail_report()
            .ok_or_else(|| ProviderError::invalid_state("fake provider cannot report"))?
            .accept(report)?;
        self.reports.lock().unwrap().push(report.clone());
        Ok(ReportReceipt::new(report.target.clone()))
    }
}

#[async_trait::async_trait]
impl engine_api::MailboxWrites for FakeProvider {
    async fn edit_mailbox(
        &self,
        _account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("account unreachable"));
        }
        let Some(tree) = &self.tree else {
            return Err(ProviderError::invalid_state("no folder writes"));
        };
        let mut folders = tree.lock().unwrap();
        let mut place = |target: &MailboxId, name: &str, parent: Option<&MailboxId>| {
            let folder = folders.iter_mut().find(|m| &m.id == target).unwrap();
            folder.name = name.to_owned();
            folder.parent = parent.cloned();
        };
        Ok(match edit {
            MailboxEdit::Create {
                name, parent, role, ..
            } => {
                let id = MailboxId::try_from(format!("made-{name}").as_str()).unwrap();
                let mut made = Mailbox::new(id.clone(), name.clone());
                made.parent.clone_from(parent);
                made.role.clone_from(role);
                folders.push(made);
                MailboxEditReceipt::resolved(id)
            }
            MailboxEdit::Update {
                target,
                name,
                parent,
            } => {
                place(target, name, parent.as_ref());
                MailboxEditReceipt::resolved(target.clone())
            }
            MailboxEdit::Trash {
                target,
                trash,
                name,
            } => {
                place(target, name, Some(trash));
                MailboxEditReceipt::resolved(target.clone())
            }
            MailboxEdit::Delete { target } => {
                folders.retain(|m| &m.id != target);
                MailboxEditReceipt::removed()
            }
        })
    }
}
impl engine_api::CalendarSync for FakeProvider {}
impl CalendarWrites for FakeProvider {}
