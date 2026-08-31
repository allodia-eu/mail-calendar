//! Background body pre-fetch; warming the store's body + blob caches for offline reading.
//!
//! The list sync fetches only message metadata (envelope, structure, flags), so without this
//! pass a message's body arrives the first time it is *opened*: a network round trip that,
//! on a slow server, stalls the reading view for seconds and fails outright with no
//! connection. After **every** mail sync (account add, boot/periodic refresh, reconnect,
//! sync-depth change, an IDLE watch delivery), [`App::prefetch_account_bodies`] drains the
//! engine's missing-body work list newest-first, pulling each body through
//! [`engine_api::Engine::message_body`], which caches the raw source on disk and the
//! extracted text in SQLite. Once warmed, an open in the synced window is a local read, the
//! window is readable offline, and body search covers it; bar the messages over
//! [`default_prefetch_size_limit`], which are left for the open that asks for them. A second
//! `impl App` block over the runtime's fields, kept out of `sync.rs` for the size limit.

use std::{collections::HashSet, time::Instant};

use engine_api::{AccountId, Provider, ProviderKey};

use crate::{App, form_factor::FormFactor};

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
/// syncs in mid-pass.
const PREFETCH_BATCH: usize = 200;

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

impl<P: Provider> App<P> {
    /// Warms the store's body cache for the messages in `account`'s synced window,
    /// newest-first, so opens are instant and the window is readable offline.
    ///
    /// Fetches run [`ConnectionInfo::concurrent_fetches`](engine_api::ConnectionInfo) at a
    /// time: the transport's own answer to how many single-object fetches are worth having in
    /// flight. It is 1 for a session protocol whose commands share one socket (IMAP), which is
    /// the case this loop used to assume for everyone; an HTTP adapter multiplexes over a
    /// pooled connection and says so, and there the difference is the whole cost of a first
    /// sync, because each body is one round trip of latency that nothing else was overlapping.
    /// Messages above the account's message-size cap are left for the on-demand open.
    ///
    /// Single-flight per account: a pass started while another is already
    /// draining the same account returns immediately (the running pass re-queries the missing
    /// set until it is empty, so it picks up what a newer sync added). Best-effort: a failed
    /// body is skipped for the rest of the pass (a later pass, or the on-demand open;
    /// retries it), and the walk stops early if the app goes offline or the account is removed
    /// mid-warm. A no-op offline, for an unknown account, or once the cache is already warm;
    /// an all-warm pass is one key scan in the engine, fetching and deserializing nothing.
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
        let mut warmed = 0usize;
        let mut first_error: Option<String> = None;
        // Keys already tried this pass. A body whose fetch failed stays in the engine's
        // missing set, so without this a permanently failing message (e.g. expunged on the
        // server behind a stale cursor) would make the drain loop spin forever.
        let mut attempted: HashSet<ProviderKey> = HashSet::new();
        // Folders already re-synced this pass after a fetch **conflict**. A conflict means
        // the folder's state moved under its stored keys (an IMAP `UIDVALIDITY` renumbering:
        // every key in the folder is stale, and (for a folder synced on demand) nothing
        // else ever re-syncs it, so its bodies would fail on every pass forever). The
        // recovery the engine documents is "re-sync, then retry": re-sync that one folder
        // once per pass; the re-snapshot replaces the stale keys, and the drain loop's next
        // query picks the fresh ones up and warms them.
        let mut resynced_folders: HashSet<String> = HashSet::new();
        // How many bodies to have in flight, asked of the transport rather than assumed. An
        // account with no provider cannot warm anything, so the pass ends before it matters.
        let Some(concurrency) = self.account_handle(account).await.and_then(|handle| {
            handle
                .providers
                .first()
                .map(|p| p.connection_info().concurrent_fetches)
        }) else {
            return;
        };
        // Read once, so a limit changed mid-drain does not apply to half a pass. Per account,
        // because it is the account's own setting.
        let size_limit = self.effective_message_size_limit(account.as_str());
        // Counted so the summary can say the window is not fully offline-readable, rather than
        // leaving a silently partial warm that looks complete.
        let mut skipped_large = 0usize;
        'drain: loop {
            // Widen the query as failures accumulate: the work list is newest-first, so a
            // clump of persistently failing messages would otherwise fill every batch and
            // stall the walk; older mail behind them must still warm (the bug that froze a
            // real backfill at the newest ~200 failures).
            let batch = self
                .engine
                .mail_missing_body(
                    core::slice::from_ref(account),
                    attempted.len() + PREFETCH_BATCH,
                )
                .await
                .unwrap_or_default();
            let fresh: Vec<_> = batch
                .into_iter()
                .filter(|row| attempted.insert(row.mail.key.clone()))
                .collect();
            if fresh.is_empty() {
                break;
            }
            // Bodies are fetched with the accounts read guard released; `account_handle`
            // hands back an `Arc` clone, and the handle is re-taken per wave rather than per
            // message, so a removal or reconnect is still seen within a few hundred
            // milliseconds without re-locking on every body.
            for wave in fresh.chunks(concurrency.max(1)) {
                // Offline now, or the account was removed mid-warm: stop: a later sync
                // re-warms.
                if !self.is_online() {
                    break 'drain;
                }
                let Some(handle) = self.account_handle(account).await else {
                    break 'drain;
                };
                let Some(provider) = handle.providers.first() else {
                    break 'drain;
                };

                // The work list is rows; fetching a body needs the whole normalised message
                // (its MIME structure and blob ids), so it is resolved here; one indexed read
                // beside a network round trip. Resolved before the fan-out because the size a
                // message reports is what decides whether it is fetched at all.
                let mut targets = Vec::with_capacity(wave.len());
                for row in wave {
                    let Some(message) = self
                        .engine
                        .messages_by_keys(account, std::slice::from_ref(&row.mail.key))
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .next()
                    else {
                        continue;
                    };
                    // An absent size is no opinion, never "small": only a size we have and
                    // that exceeds the cap defers the body to the open that asks for it.
                    if size_limit.is_some_and(|cap| message.size.is_some_and(|size| size > cap)) {
                        skipped_large += 1;
                        continue;
                    }
                    targets.push((row, message));
                }

                // The wave is already the width of the fetch window, so joining it *is* the
                // bound: no need for a buffered stream on top, and the simpler future keeps
                // this whole call `Send` for the hosts that spawn it.
                let outcomes = futures::future::join_all(targets.into_iter().map(
                    |(row, message)| async move {
                        // Two calls, because a body read is text-first: it returns without
                        // touching the bytes once the text is cached, which leaves a message
                        // whose source a lowered cap dropped on the work list for ever. The
                        // second costs one indexed read where there is nothing to fetch.
                        let result =
                            match self.engine.message_body(provider, account, &message).await {
                                Ok(body) => self
                                    .engine
                                    .ensure_message_source(provider, account, &message)
                                    .await
                                    .map(|()| body),
                                Err(err) => Err(err),
                            };
                        (row, result)
                    },
                ))
                .await;

                // Outcomes are folded in sequentially: the counters, the hint and the
                // one-re-sync-per-folder recovery are all shared state, and a conflict recovery
                // re-syncs a whole folder, which must not happen once per message that hit it.
                for (row, result) in outcomes {
                    match result {
                        Ok(_) => {
                            warmed += 1;
                            // Report the warm to the status hint. Not every body: the hint is a
                            // line of text, the signal it raises costs every client a snapshot
                            // pull, and this loop runs thousands of times on a first sync;
                            // which is the shape that pegged a core and starved the UI thread
                            // before the observer learned to coalesce. Once a second's worth of
                            // progress is plenty for a caption that only says "still going".
                            if warmed.is_multiple_of(PREFETCH_HINT_EVERY) {
                                self.note_warming(account, Some(warmed_count(warmed)));
                            }
                        }
                        Err(err) => {
                            // A conflict is recoverable: re-sync the message's folder (its keys
                            // are stale) via the shared one-folder re-sync, which also rebuilds
                            // the snapshot, since the folder's row keys change, and the loop's
                            // next query returns the fresh keys.
                            if err.is_conflict()
                                && let Some(folder) = row.mailboxes.first()
                                && resynced_folders.insert(folder.as_str().to_owned())
                            {
                                self.resync_folder(account, folder.as_str(), "body-conflict")
                                    .await;
                                continue;
                            }
                            // Record the first failure's shape for the summary line below;
                            // engine errors carry protocol/folder context, never message
                            // content, so this stays within the logging privacy rule.
                            if first_error.is_none() {
                                first_error = Some(err.to_string());
                            }
                        }
                    }
                }
            }
        }
        // Skip the log when there was nothing to do: this runs after every sync, and a
        // steady-state no-op pass per poll tick would drown the diagnostic log.
        if !attempted.is_empty() {
            log::info!(
                "prefetch: warmed {warmed}/{} bodies in {}ms, {concurrency} at a time",
                attempted.len(),
                start.elapsed().as_millis(),
            );
        }
        if skipped_large > 0 {
            log::info!(
                "prefetch: left {skipped_large} large message(s) for the open that asks for them",
            );
        }
        if let Some(err) = first_error {
            // Deliberately skipped bodies are not failures; counting them here would report a
            // mailbox of large messages as a mailbox that could not be warmed.
            log::warn!(
                "prefetch: {} bodies failed; first error: {err}",
                attempted.len().saturating_sub(warmed + skipped_large),
            );
        }
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
