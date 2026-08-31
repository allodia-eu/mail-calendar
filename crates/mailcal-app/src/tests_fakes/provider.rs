//! The JMAP-shaped [`FakeProvider`] test fixture and its builder variants. Split out of
//! `tests_fakes.rs` to keep each file under the size limit; a submodule of the shared
//! `fakes` module.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use engine_api::AccountId;
use engine_core::{
    ids::MailboxId,
    mail::{Mailbox, MailboxRole, Message},
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, MailEdit, MailEditReceipt,
    MessageReport, Provider, ProviderError, ProviderResult, ReportReceipt, ScopeSync,
};
use tokio::sync::Notify;

use super::message;

#[allow(clippy::duplicate_mod)]
#[path = "provider_builders.rs"]
mod builders;

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
    /// Message keys whose [`Provider::fetch_message_source`] fails with a **conflict**;
    /// models stale keys (an IMAP `UIDVALIDITY` renumbering), so a test can prove a
    /// body-warm pass looks past them and triggers the folder re-sync recovery.
    source_failures: Vec<String>,
}

/// Decrements the in-flight count however a source fetch leaves.
struct InFlightGuard(Arc<Mutex<(usize, usize)>>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.lock().unwrap().0 -= 1;
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
        ConnectionInfo::new(self.caps).with_concurrent_fetches(self.concurrent_fetches)
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
        _window: SyncWindow,
        _fetch_batch: usize,
        chunk_size: usize,
    ) -> EmailStream<'a> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
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
            pages(&self.messages, chunk_size)
                .map(|page| {
                    EmailChunk::additive(
                        page.to_vec(),
                        Vec::new(),
                        Some(self.messages.len()),
                        SyncState::new("email-1"),
                    )
                })
                .collect()
        } else {
            reconcile_chunks(&self.messages, chunk_size)
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
        // A downed provider can't apply an edit either: the same modelling
        // `fetch_message_source` already does, so a test can prove a refused write surfaces as
        // `MailActionError::Rejected` instead of a silent success.
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProviderError::retryable("account unreachable"));
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
        if self
            .source_failures
            .iter()
            .any(|k| k == message.id.key().as_str())
        {
            return Err(ProviderError::conflict(
                "UIDVALIDITY changed for a: re-sync before retrying",
            ));
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

/// Splits `messages` the way a real adapter yields them: `chunk_size` per chunk, `0` meaning
/// one chunk for the lot (what [`StreamTuning`](engine_api::StreamTuning) documents).
///
/// A fake that hands the whole mailbox over in one chunk cannot see the core's commit
/// granularity at all, and that granularity is what a streamed pass costs, because the core
/// re-projects its cached list once per commit. So the parameter under test is honoured here
/// rather than ignored.
///
/// An empty mailbox still yields one (empty) chunk: a pass that finds nothing still has a final
/// chunk to reconcile the store against.
fn pages(messages: &[Message], chunk_size: usize) -> impl Iterator<Item = &[Message]> {
    let pages: Vec<&[Message]> = match chunk_size {
        _ if messages.is_empty() => vec![&[]],
        0 => vec![messages],
        size => messages.chunks(size).collect(),
    };
    pages.into_iter()
}

/// `messages` as a reconciling pass's chunks: intermediate pages hold the cursor and carry the
/// keys they cover, and only the last advances it and tombstones what no page named.
fn reconcile_chunks(messages: &[Message], chunk_size: usize) -> Vec<EmailChunk> {
    let total = messages.len();
    let pages: Vec<&[Message]> = pages(messages, chunk_size).collect();
    let last = pages.len() - 1;
    pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            let present = page.iter().map(|m| m.id.key().clone()).collect();
            if index == last {
                EmailChunk::reconcile_last(
                    page.to_vec(),
                    present,
                    Some(total),
                    SyncState::new("email-1"),
                )
            } else {
                EmailChunk::reconcile_page(page.to_vec(), present, Some(total))
            }
        })
        .collect()
}
