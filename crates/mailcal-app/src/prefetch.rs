//! Background body pre-fetch; warming the store's body + blob caches for offline reading.
//!
//! The list sync fetches only message metadata (envelope, structure, flags), so without this
//! pass a message's body arrives the first time it is *opened*: a network round trip that,
//! on a slow server, stalls the reading view for seconds and fails outright with no
//! connection. After **every** mail sync (account add, boot/periodic refresh, reconnect,
//! sync-depth change, an IDLE watch delivery), [`App::prefetch_account_bodies`] drains the
//! engine's missing-body work list newest-first, in batches through
//! [`engine_api::Engine::warm_message_sources`], which caches each raw source on disk and its
//! extracted text in SQLite as it arrives. Once warmed, an open in the synced window is a
//! local read, the window is readable offline, and body search covers it; bar the messages
//! over [`default_prefetch_size_limit`], which are left for the open that asks for them. A
//! second `impl App` block over the runtime's fields, kept out of `sync.rs` for the size limit.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use engine_api::{AccountId, Message, Provider, ProviderKey};
use futures::{StreamExt, stream::FuturesUnordered};

use crate::{
    App,
    connectivity::{is_throttled, throttled_for},
    form_factor::FormFactor,
};

/// Holds an account in the status hint's body phase for as long as a warm is running, and takes
/// it out however the warm ends.
///
/// A guard rather than a call at the end, because the drain loop leaves by four other doors;
/// offline, account removed, no provider, nothing left to warm, and an account left in the hint
/// would claim to be catching up for the rest of the session.
struct WarmingHint<'a, P: Provider> {
    app: &'a App<P>,
    account: &'a AccountId,
}

impl<P: Provider> Drop for WarmingHint<'_, P> {
    fn drop(&mut self) {
        self.app.note_warming(self.account, None);
    }
}

/// A warmed count as the hint carries it, saturating rather than wrapping on a mailbox no one
/// has.
fn warmed_count(warmed: usize) -> u32 {
    u32::try_from(warmed).unwrap_or(u32::MAX)
}

/// How many missing-body work items one engine query returns. A bound on **memory** (each
/// item is a fully deserialized `Message`), not on coverage: the pass re-queries until the
/// missing set drains, so the entire synced window warms; re-querying also picks up mail that
/// syncs in mid-pass. Large enough that a page keeps every connection of a batching transport
/// busy for several batches before the next query.
const PREFETCH_BATCH: usize = 500;

/// How many warmed bodies pass between status-hint updates.
///
/// The hint is one line of text, and the signal it raises costs every client a snapshot pull, so
/// it is reported at a rate a reader can use rather than once per body: this loop runs thousands
/// of times on a first sync.
const PREFETCH_HINT_EVERY: usize = 25;

/// The largest message the warm pulls in full where this build runs, in octets; `None` warms
/// every size.
///
/// Which side of the trade a device is on is the form factor's call: a laptop spends disk, a
/// phone would spend a metered link: so this only forwards it. It is the value an account
/// with no explicit setting of its own resolves to
/// ([`App::effective_message_size_limit`](crate::App::effective_message_size_limit)).
///
/// A *message* whose size is `None` is a different thing entirely: an adapter with **no
/// opinion**, never "small", so those are always fetched. Treating an unreported size as
/// oversized would silently stop warming every message a provider does not measure.
#[must_use]
pub fn default_prefetch_size_limit() -> Option<u64> {
    FormFactor::current().default_prefetch_size_limit()
}

/// How long a warm waits after a throttle that named no wait, before asking again.
///
/// The engine hands a throttle back without an instant when the server gave none, and leaves the
/// schedule to the host. The servers measured doing that count per minute: Stalwart refuses an
/// account past about 1,000 requests a minute with a bare `429`, and Gmail's quotas are stated
/// per minute. A minute is therefore the wait after which the window has certainly moved on.
const UNSTATED_THROTTLE_PAUSE: Duration = Duration::from_mins(1);

/// The longest wait a warm sleeps on inside one pass. A server asking for longer has made a
/// scheduling decision, and a background task asleep for longer than this is
/// indistinguishable from a hang: the rest waits for the next pass.
const LONGEST_THROTTLE_PAUSE: Duration = Duration::from_mins(5);

/// How many throttles one pass waits out before leaving the rest for the next pass.
const THROTTLE_PAUSES_PER_PASS: usize = 3;

/// One batch of a warm: the messages asked for in one provider request, and the folder each
/// sits in, for the re-sync a stale key calls for.
struct Batch {
    messages: Vec<Message>,
    folders: Vec<Option<String>>,
}

/// How a round of batches ended.
enum Round {
    /// Every batch ran.
    Done,
    /// The pass must stop: offline, or the account is gone.
    Stopped,
    /// The server refused for now. `keys` are the messages it refused and those not yet asked
    /// for; `stated` is the longest wait any refusal named.
    Throttled {
        keys: Vec<ProviderKey>,
        stated: Option<Duration>,
    },
}

/// What one warming pass has done so far.
#[derive(Default)]
struct Pass {
    /// Keys already tried this pass. A body whose fetch failed stays in the engine's missing
    /// set, so without this a permanently failing message (e.g. expunged on the server behind a
    /// stale cursor) would make the drain loop spin forever.
    attempted: HashSet<ProviderKey>,
    /// Folders already re-synced this pass after a fetch **conflict**. A conflict means the
    /// folder's state moved under its stored keys (an IMAP `UIDVALIDITY` renumbering: every key
    /// in the folder is stale, and for a folder synced on demand nothing else ever re-syncs it,
    /// so its bodies would fail on every pass forever). The recovery the engine documents is
    /// "re-sync, then retry": re-sync that one folder once per pass; the re-snapshot replaces
    /// the stale keys, and the drain loop's next query picks the fresh ones up and warms them.
    resynced: HashSet<String>,
    warmed: usize,
    /// Counted so the summary can say the window is not fully offline-readable, rather than
    /// leaving a silently partial warm that looks complete.
    skipped_large: usize,
    /// The first failure's shape, for the summary line; engine errors carry protocol and folder
    /// context, never message content, so this stays within the logging privacy rule.
    first_error: Option<String>,
}

impl<P: Provider> App<P> {
    /// Warms the store's body cache for the messages in `account`'s synced window,
    /// newest-first, so opens are instant and the window is readable offline.
    ///
    /// Two facts from the transport shape the pass, both read from
    /// [`ConnectionInfo`](engine_api::ConnectionInfo) rather than assumed:
    /// `sources_per_request`, how many bodies one request can carry (an IMAP `UID FETCH` over a
    /// set), and `concurrent_fetches`, how many requests are worth having in flight (IMAP: the
    /// account's pooled connections; HTTP: what the service tolerates). Each page of the work
    /// list is grouped by folder, cut into batches of the first, and run with the second
    /// in flight, the next batch starting as soon as any finishes, so one slow batch holds one
    /// connection rather than the pass. On a first sync this is the whole cost: each request
    /// is a round trip of latency, and the bytes themselves arrive in a fraction of it.
    /// Messages above the account's message-size cap are left for the on-demand open.
    ///
    /// Single-flight per account: a pass started while another is already
    /// draining the same account returns immediately (the running pass re-queries the missing
    /// set until it is empty, so it picks up what a newer sync added). Best-effort: a failed
    /// body is skipped for the rest of the pass (a later pass, or the on-demand open,
    /// retries it), and no new batch starts once the app goes offline or the account is
    /// removed mid-warm; the batches already out finish. A **throttled** body is not a failed
    /// one: the pass stops asking, waits the server's stated time (or
    /// [`UNSTATED_THROTTLE_PAUSE`] where it named none), and asks again for what was refused
    /// and what it had not reached, up to [`THROTTLE_PAUSES_PER_PASS`] times and never
    /// sleeping past [`LONGEST_THROTTLE_PAUSE`]. A no-op offline, for an unknown
    /// account, or once the cache is already warm; an all-warm pass is one key scan in the
    /// engine, fetching and deserializing nothing.
    pub(crate) async fn prefetch_account_bodies(&self, account: &AccountId) {
        if !self.is_online() {
            return;
        }
        let Some(_guard) = self.begin_prefetch(account) else {
            return;
        };
        let start = Instant::now();
        // Clears on every exit path below, including the early `break 'drain`s: an account left
        // in the hint would say it was catching up for the rest of the session.
        let _hint = WarmingHint { app: self, account };
        // An account with no provider cannot warm anything, so the pass ends before it matters.
        let Some(info) = self
            .account_handle(account)
            .await
            .and_then(|handle| handle.providers.first().map(Provider::connection_info))
        else {
            return;
        };
        let width = info.concurrent_fetches.max(1);
        let per_request = info.sources_per_request.max(1);
        // Read once, so a limit changed mid-drain does not apply to half a pass. Per account,
        // because it is the account's own setting.
        let size_limit = self.effective_message_size_limit(account.as_str());
        let mut pass = Pass::default();
        let mut pauses = 0usize;
        loop {
            // Widen the query as failures accumulate: the work list is newest-first, so a
            // clump of persistently failing messages would otherwise fill every page and
            // stall the walk; older mail behind them must still warm (the bug that froze a
            // real backfill at the newest ~200 failures).
            let page = self
                .engine
                .mail_missing_body(
                    core::slice::from_ref(account),
                    pass.attempted.len() + PREFETCH_BATCH,
                )
                .await
                .unwrap_or_default();
            // In the page's own order, which is newest-first.
            let mut folders: Vec<(ProviderKey, Option<String>)> = Vec::new();
            for row in page {
                if pass.attempted.insert(row.mail.key.clone()) {
                    let folder = row.mailboxes.first().map(|id| id.as_str().to_owned());
                    folders.push((row.mail.key, folder));
                }
            }
            if folders.is_empty() {
                break;
            }
            let batches = self
                .plan_batches(account, folders, size_limit, per_request, &mut pass)
                .await;
            let mut conflicted = HashSet::new();
            let round = self
                .run_batches(account, batches, width, &mut pass, &mut conflicted)
                .await;
            // Once per folder per pass, after the page's batches are in: a re-sync rebuilds the
            // folder's keys, and the loop's next query returns the fresh ones.
            for folder in conflicted {
                if pass.resynced.insert(folder.clone()) {
                    self.resync_folder(account, &folder, "body-conflict").await;
                }
            }
            match round {
                Round::Done => {}
                Round::Stopped => break,
                Round::Throttled { keys, stated } => {
                    // Back on the work list, so the next query asks for them again.
                    for key in &keys {
                        pass.attempted.remove(key);
                    }
                    let pause = stated.unwrap_or(UNSTATED_THROTTLE_PAUSE);
                    if pauses == THROTTLE_PAUSES_PER_PASS || pause > LONGEST_THROTTLE_PAUSE {
                        log::info!(
                            "prefetch: the server is limiting requests; {} bodies are left for \
                             the next pass",
                            keys.len(),
                        );
                        break;
                    }
                    pauses += 1;
                    log::info!(
                        "prefetch: the server is limiting requests ({}); resuming in {}s",
                        if stated.is_some() {
                            "wait stated"
                        } else {
                            "no wait stated"
                        },
                        pause.as_secs(),
                    );
                    tokio::time::sleep(pause).await;
                    if !self.is_online() {
                        break;
                    }
                }
            }
        }
        // A warmed body is what gives a row its preview snippet on a provider that sends none
        // (IMAP: the engine derives one from the body it just fetched), so the list on screen is
        // out of date the moment this pass warms anything. Once per pass, not per body: the
        // rows arrive together and this loop runs thousands of times on a first sync. Without
        // it the previews waited for whatever rebuilt the list next, which for a folder nobody
        // else syncs meant opening a message and coming back.
        if pass.warmed > 0 {
            self.invalidate_list_cache();
            self.rebuild_snapshot().await;
        }
        // Skip the log when there was nothing to do: this runs after every sync, and a
        // steady-state no-op pass per poll tick would drown the diagnostic log.
        if !pass.attempted.is_empty() {
            log::info!(
                "prefetch: warmed {}/{} bodies in {}ms, {width} requests at a time of up to \
                 {per_request}",
                pass.warmed,
                pass.attempted.len(),
                start.elapsed().as_millis(),
            );
        }
        if pass.skipped_large > 0 {
            log::info!(
                "prefetch: left {} large message(s) for the open that asks for them",
                pass.skipped_large,
            );
        }
        if let Some(err) = pass.first_error {
            // Deliberately skipped bodies are not failures; counting them here would report a
            // mailbox of large messages as a mailbox that could not be warmed.
            log::warn!(
                "prefetch: {} bodies failed; first error: {err}",
                pass.attempted
                    .len()
                    .saturating_sub(pass.warmed + pass.skipped_large),
            );
        }
    }

    /// Resolves a page of work items to the messages a fetch needs (their MIME structure and
    /// blob ids) in one indexed read, leaves those over `size_limit` for the open that asks
    /// for them, and cuts the rest into batches of `per_request`, grouped by folder, so that
    /// on IMAP a batch is one mailbox's request rather than several. The page's newest-first
    /// order survives: each folder's batch fills in page order, and batches run in the order
    /// their newest message has in the page.
    async fn plan_batches(
        &self,
        account: &AccountId,
        folders: Vec<(ProviderKey, Option<String>)>,
        size_limit: Option<u64>,
        per_request: usize,
        pass: &mut Pass,
    ) -> Vec<Batch> {
        let keys: Vec<ProviderKey> = folders.iter().map(|(key, _)| key.clone()).collect();
        // The read returns messages in an order of its own, not the page's.
        let mut resolved: HashMap<ProviderKey, Message> = self
            .engine
            .messages_by_keys(account, &keys)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|message| (message.id.key().clone(), message))
            .collect();
        let mut batches: Vec<Batch> = Vec::new();
        // The batch each folder is filling, by its position in `batches`.
        let mut filling: HashMap<Option<String>, usize> = HashMap::new();
        for (key, folder) in folders {
            let Some(message) = resolved.remove(&key) else {
                continue;
            };
            // An absent size is no opinion, never "small": only a size we have and that
            // exceeds the cap defers the body to the open that asks for it.
            if size_limit.is_some_and(|cap| message.size.is_some_and(|size| size > cap)) {
                pass.skipped_large += 1;
                continue;
            }
            let open = filling
                .get(&folder)
                .copied()
                .filter(|&at| batches[at].messages.len() < per_request);
            if let Some(at) = open {
                batches[at].messages.push(message);
                batches[at].folders.push(folder);
            } else {
                filling.insert(folder.clone(), batches.len());
                batches.push(Batch {
                    messages: vec![message],
                    folders: vec![folder],
                });
            }
        }
        batches
    }

    /// Runs `batches` with up to `width` in flight, folding each batch's outcomes into `pass`
    /// as it completes and naming in `conflicted` the folders whose keys went stale.
    ///
    /// The account's provider is taken afresh for every batch, so a reconnect is seen by the
    /// next batch and a removal stops the pass within one. No new batch starts once the app
    /// is offline, the account is gone, or the server has refused a message as throttled: the
    /// batches already out are waited for rather than dropped, because a request abandoned
    /// mid-response leaves its connection to be drained by whoever borrows it next.
    async fn run_batches(
        &self,
        account: &AccountId,
        batches: Vec<Batch>,
        width: usize,
        pass: &mut Pass,
        conflicted: &mut HashSet<String>,
    ) -> Round {
        let mut queue = batches.into_iter();
        let mut running = FuturesUnordered::new();
        let mut stopped = false;
        // Refused as throttled, and the longest wait any refusal named.
        let mut held: Vec<ProviderKey> = Vec::new();
        let mut stated: Option<Duration> = None;
        loop {
            while !stopped && held.is_empty() && running.len() < width {
                let Some(batch) = queue.next() else {
                    break;
                };
                let handle = self.account_handle(account).await;
                let Some(handle) = handle.filter(|_| self.is_online()) else {
                    stopped = true;
                    break;
                };
                running.push(async move {
                    let Some(provider) = handle.providers.first() else {
                        return (batch, Vec::new());
                    };
                    let outcomes: Vec<_> = self
                        .engine
                        .warm_message_sources(provider, account, &batch.messages)
                        .collect()
                        .await;
                    (batch, outcomes)
                });
            }
            let Some((batch, outcomes)) = running.next().await else {
                break;
            };
            let folders = &batch.folders;
            for (index, outcome) in outcomes {
                match outcome {
                    // Not a failure of the message: the server declined to serve anyone for
                    // now. Held for the pause the caller takes, rather than dropped to a pass
                    // that may be half an hour away.
                    Err(err) if is_throttled(&err) => {
                        if let Some(message) = batch.messages.get(index) {
                            held.push(message.id.key().clone());
                        }
                        stated = stated.max(throttled_for(&err));
                    }
                    Ok(()) => {
                        pass.warmed += 1;
                        // Report the warm to the status hint. Not every body: the hint is a
                        // line of text, the signal it raises costs every client a snapshot
                        // pull, and this loop runs thousands of times on a first sync.
                        if pass.warmed.is_multiple_of(PREFETCH_HINT_EVERY) {
                            self.note_warming(account, Some(warmed_count(pass.warmed)));
                        }
                    }
                    Err(err) => {
                        let folder = folders.get(index).cloned().flatten();
                        // A folder already re-synced this pass gets no second one, so its
                        // conflict is a failure like any other.
                        match folder {
                            Some(folder)
                                if err.is_conflict() && !pass.resynced.contains(&folder) =>
                            {
                                conflicted.insert(folder);
                            }
                            _ => {
                                if pass.first_error.is_none() {
                                    pass.first_error = Some(err.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        if stopped {
            return Round::Stopped;
        }
        if held.is_empty() {
            return Round::Done;
        }
        held.extend(queue.flat_map(|batch| batch.messages.into_iter().map(|m| m.id.key().clone())));
        Round::Throttled { keys: held, stated }
    }

    /// Marks `account` as having a warming pass in flight, or returns `None` if one already
    /// is. The returned guard clears the mark when dropped, on every exit path.
    fn begin_prefetch(&self, account: &AccountId) -> Option<PrefetchGuard<'_>> {
        let mut in_flight = self.prefetching.lock().expect("prefetching mutex poisoned");
        if !in_flight.insert(account.as_str().to_owned()) {
            return None;
        }
        Some(PrefetchGuard {
            app: &self.prefetching,
            account: account.as_str().to_owned(),
        })
    }
}

/// Clears an account's in-flight prefetch mark on drop, so an early return or error path can
/// never leave the account permanently "already warming".
struct PrefetchGuard<'a> {
    app: &'a std::sync::Mutex<HashSet<String>>,
    account: String,
}

impl Drop for PrefetchGuard<'_> {
    fn drop(&mut self) {
        self.app
            .lock()
            .expect("prefetching mutex poisoned")
            .remove(&self.account);
    }
}
