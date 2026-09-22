//! Per-account sync of a single account's folders, plus reachability classification.
//!
//! Split out of `sync.rs` to keep each file under the size limit. [`sync_account_providers`]
//! hands the account's providers to the engine, which syncs the folder list once and fans the
//! folders out itself, and turns the per-scope report back into the two product answers only
//! this layer can give: whether the account reached its server ([`Reach`], for the outage badge)
//! and whether its sign-in was refused. It also writes the pass to the diagnostic log.

use std::time::Duration;

use engine_api::{Engine, MailSyncReport, Provider, StreamTuning, SyncError, SyncObserver};

use crate::{
    Account,
    connectivity::{is_signin_expired, is_throttled, throttled_for},
};

/// The result of one per-account sync pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncAccountOutcome {
    /// Whether this pass reached the server (`None` means every attempted scope was busy).
    pub(crate) reachable: Option<bool>,
    /// Whether the account's stored credential was **refused with nothing else working** this pass
    /// ([`signin_expired`]) (`Some(true)` raises the "your sign-in expired) reconnect" prompt,
    /// `Some(false)` clears it, `None` leaves it alone (nothing this pass proves either way).
    pub(crate) signin_expired: Option<bool>,
    /// How many scopes were skipped because another sync already held them.
    pub(crate) busy_scopes: usize,
    /// The longest wait any scope's provider **named** this pass, where one did.
    ///
    /// The longest, because coming back before the furthest-out instant just meets the same
    /// refusal again; and only where the server named one, since the engine reports an instant
    /// exactly when it declined to absorb the wait itself (`http-throttling.md`). `None` means
    /// nothing was throttled, or nothing said when. Either way the caller keeps its own
    /// schedule.
    pub(crate) throttled_for: Option<Duration>,
    /// Whether the account's server **refused this pass for now**: `Some(true)` raises the
    /// paused notice, `Some(false)` clears it, `None` leaves it alone ([`throttled`]).
    ///
    /// Separate from [`throttled_for`](Self::throttled_for) because the two answer different
    /// questions and neither implies the other: about two Gmail refusals in three state no
    /// instant at all, so a pause with no figure is the common case, not an edge one.
    pub(crate) throttled: Option<bool>,
}

/// Syncs one account's mail **concurrently**: sync the folder list **once**, then stream
/// **every folder's email in parallel** (distinct per-folder scopes never contend).
/// A free function (not a method) so callers can run it while holding
/// the `accounts` read guard, and so several accounts can run it at once under one `join_all`.
///
/// Reports the account's **reachability** this pass for its outage badge
/// ([`App::set_account_reachable`]): `Some(true)` if the folder list or any folder synced,
/// `Some(false)` if every network op failed with a real error (server down, no route, revoked
/// credentials), or `None` when the result is **indeterminate**; every op was skipped because
/// a concurrent sync already held the scope ([`ApiError::Busy`]), which says nothing about
/// reachability, so the caller leaves the existing badge untouched.
pub(crate) async fn sync_account_providers<P: Provider, K: SyncObserver>(
    engine: &Engine,
    account: &Account<P>,
    tuning: StreamTuning,
    observer: &K,
    acct: usize,
) -> SyncAccountOutcome {
    // `acct` is a per-pass ordinal (not the account id, which carries the address) so the
    // interleaved timing of concurrent accounts stays attributable without logging identity.
    if account.providers.is_empty() {
        log::info!("sync[a{acct}]: skipped; no live mail providers");
        return SyncAccountOutcome {
            reachable: None,
            signin_expired: None,
            busy_scopes: 0,
            throttled_for: None,
            throttled: None,
        };
    }

    // The engine owns the fan-out: the folder list once, then the folders bounded and Inbox
    // first. What comes back is per scope, which is the whole reason this can still tell an
    // outage from a refused credential from a scope another pass is holding.
    let report = engine
        .sync_mail(&account.providers, &account.id, tuning, observer)
        .await;

    // `None` means this pass never looked at the folder list, which says nothing about whether
    // the server answered: the same standing as a scope another pass was holding.
    let list_reach = report.mailboxes.as_ref().map_or(Reach::Busy, reach_of);
    let folder_reaches: Vec<Reach> = report.folders.iter().map(|f| reach_of(&f.result)).collect();

    log_pass(acct, &report, &folder_reaches);

    SyncAccountOutcome {
        reachable: reachability(list_reach, folder_reaches.iter().copied()),
        signin_expired: signin_expired(list_reach, folder_reaches.iter().copied()),
        busy_scopes: usize::from(list_reach == Reach::Busy)
            + folder_reaches.iter().filter(|r| **r == Reach::Busy).count(),
        throttled_for: longest_stated_wait(&report),
        throttled: throttled(list_reach, folder_reaches.iter().copied()),
    }
}

/// Writes the pass to the diagnostic log: one line for the account, one per folder that did
/// something worth knowing about.
///
/// Counts, durations and scope **positions** only; never a folder name, an address or an id
/// (`docs/logging.md`). A folder is named by its index in the pass, which is enough to line a
/// slow or failing one up against the account summary above it without putting the user's
/// mailbox names in a file they hand to support.
fn log_pass(acct: usize, report: &MailSyncReport, reaches: &[Reach]) {
    if let Err(err) = &report.account_steps {
        // Deliberately its own line, and deliberately not phrased as an outage: this is the
        // store failing, and reading it as "the server is down" is how a schema problem sends
        // someone to check their wifi.
        log::warn!("sync[a{acct}]: local store step failed: {err}");
    }
    if let Some(Err(err)) = &report.mailboxes
        && !err.is_busy()
    {
        log::warn!("sync[a{acct}]: folder list failed: {err}");
    }

    for (index, (folder, reach)) in report.folders.iter().zip(reaches).enumerate() {
        let ms = folder.elapsed.as_millis();
        // Where the time went, for a folder that did something. The three phases are parts of
        // `ms`, not a partition of it: the rest is the scope lease and the per-chunk
        // bookkeeping: so they are printed as parts and the reader can see the remainder.
        let split = |t: engine_api::SyncTiming| {
            format!(
                " (fetch {}ms, derive {}ms, store {}ms)",
                t.fetching.as_millis(),
                t.deriving.as_millis(),
                t.storing.as_millis(),
            )
        };
        match &folder.result {
            Ok(applied) if applied.upserted + applied.tombstoned > 0 => log::debug!(
                "sync[a{acct}]: folder[{index}] +{} -{} in {ms}ms{}",
                applied.upserted,
                applied.tombstoned,
                split(folder.timing),
            ),
            Ok(_) => log::debug!("sync[a{acct}]: folder[{index}] unchanged in {ms}ms"),
            Err(_) if *reach == Reach::Busy => {
                log::debug!("sync[a{acct}]: folder[{index}] busy in {ms}ms");
            }
            Err(err) => log::warn!("sync[a{acct}]: folder[{index}] failed in {ms}ms: {err}"),
        }
    }

    let busy = reaches.iter().filter(|r| **r == Reach::Busy).count();
    let failed = report.folders.len() - report.folders_synced();
    // Summed across folders, which run concurrently: so these routinely exceed the pass's own
    // wall time, and the line says "work across concurrent folders" so that reads as arithmetic
    // rather than as a bug. They measure work done; the wall time measures what the user waited
    // for. Both are worth having: one says where the time went, the other what it cost them.
    let fetching: u128 = report
        .folders
        .iter()
        .map(|f| f.timing.fetching.as_millis())
        .sum();
    let deriving: u128 = report
        .folders
        .iter()
        .map(|f| f.timing.deriving.as_millis())
        .sum();
    let storing: u128 = report
        .folders
        .iter()
        .map(|f| f.timing.storing.as_millis())
        .sum();
    log::info!(
        "sync[a{acct}]: {} folder(s), {} synced, {} msg upserted, {} removed{}{} in {}ms; \
         work across concurrent folders: fetch {fetching}ms, derive {deriving}ms, store {storing}ms",
        report.folders.len(),
        report.folders_synced(),
        report.upserted(),
        report.tombstoned(),
        if busy > 0 {
            format!(", {busy} busy")
        } else {
            String::new()
        },
        if failed > busy {
            format!(", {} failed", failed - busy)
        } else {
            String::new()
        },
        report.elapsed.as_millis(),
    );
}

/// One sync op's bearing on whether the account reached its server.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// The op succeeded: the server was reached.
    Reached,
    /// The op failed with a real error (transport, a server fault): the server was not reached.
    Unreachable,
    /// The op failed because the account's stored credential was **refused**
    /// ([`is_signin_expired`]): an expired or revoked OAuth grant. The server answered, so this
    /// is not an outage; it needs a fresh sign-in, which is a different prompt.
    Expired,
    /// The op was refused by a rate limit: the server answered, promptly, and declined to do
    /// the work yet. **Reached, not an outage**: the badge that says "can't reach the server"
    /// would send someone to check a network that is working perfectly.
    Throttled,
    /// The op was skipped because a concurrent sync held the scope ([`ApiError::Busy`]); no
    /// bearing on reachability.
    Busy,
}

/// The furthest-out instant any scope in this pass was given.
///
/// A pass fans out, so several scopes can be refused at once and each may name its own window;
/// the caller wants the one that clears last, because returning before it meets the same
/// refusal again and spends a round trip proving it.
fn longest_stated_wait(report: &MailSyncReport) -> Option<Duration> {
    let folders = report.folders.iter().map(|folder| &folder.result);
    report
        .mailboxes
        .iter()
        .chain(folders)
        .filter_map(|result| result.as_ref().err())
        .filter_map(|err| throttled_for(err))
        .max()
}

/// Classifies one sync result: success reached the server, a [`ApiError::Busy`] is a
/// concurrent-sync skip (indeterminate), a refused credential is [`Reach::Expired`], and any
/// other error is a real unreachability.
fn reach_of<T>(result: &Result<T, SyncError>) -> Reach {
    match result {
        Ok(_) => Reach::Reached,
        Err(err) if err.is_busy() => Reach::Busy,
        // `&ApiError` coerces to `&dyn Error`; the classifier walks its `source()` chain to the
        // typed provider failure and reads the engine's own class.
        Err(err) if is_signin_expired(err) => Reach::Expired,
        Err(err) if is_throttled(err) => Reach::Throttled,
        Err(_) => Reach::Unreachable,
    }
}

/// Folds the pass's op reaches into an account verdict: any success ⇒ reachable
/// (`Some(true)`), else a refused credential ⇒ **reachable** (`Some(true)`: the server answered;
/// the expired-sign-in prompt carries that story instead), else any real failure ⇒ unreachable
/// (`Some(false)`), else all-`Busy` ⇒ indeterminate (`None`, leave the badge as-is). A partial
/// failure (some folders synced, some didn't) still counts as reachable: the server responded.
///
/// [`Reach::Expired`] outranks [`Reach::Unreachable`] deliberately: a dead grant fails *every*
/// op, so a lone transport blip alongside it must not downgrade the account to a generic outage
/// and hide the one prompt that names the actual remedy.
fn reachability(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_reached = false;
    let mut any_expired = false;
    let mut any_unreachable = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            // A throttle is the server answering, so it settles reachability exactly as a
            // success does.
            Reach::Reached | Reach::Throttled => any_reached = true,
            Reach::Expired => any_expired = true,
            Reach::Unreachable => any_unreachable = true,
            Reach::Busy => {}
        }
    }
    if any_reached || any_expired {
        Some(true)
    } else if any_unreachable {
        Some(false)
    } else {
        None
    }
}

/// Whether the pass saw a refused credential **and nothing that worked**: the caller raises
/// (`Some(true)`), clears (`Some(false)`) or leaves alone (`None`) the account's expired-sign-in
/// prompt from this.
///
/// One credential serves every scope on an account, so a scope that authenticated disproves an
/// expired one: the refusal came from the server's side and costs the user nothing. Servers do
/// answer this way (a refusal deliberately delayed by ~2s is a rejection, not a timeout) and the
/// prompt is the one thing the user cannot ignore, so it needs evidence nothing else contradicts.
/// The price is that a credential expiring mid-pass is reported one pass later, once nothing
/// succeeds.
///
/// Only an unmixed success clears the prompt, for the same reason read the other way round: a
/// transport failure or a concurrent-sync skip is no evidence the credential works, and a pass
/// that both reached and was refused proves nothing about the credential the user was asked to
/// renew: so neither may retract a prompt they still have to act on.
fn signin_expired(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_reached = false;
    let mut any_expired = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            Reach::Reached => any_reached = true,
            Reach::Expired => any_expired = true,
            // A throttle proves the server answered, but says nothing about the
            // *credential*, which is refused before it is examined. So it neither raises the
            // prompt nor retracts one the user still has to act on.
            Reach::Throttled | Reach::Unreachable | Reach::Busy => {}
        }
    }
    match (any_reached, any_expired) {
        (false, true) => Some(true),
        (true, false) => Some(false),
        _ => None,
    }
}

/// Whether this pass was **refused for now** by the account's server: the caller raises
/// (`Some(true)`), clears (`Some(false)`) or leaves alone (`None`) the account's paused notice.
///
/// One refused scope is enough to raise it. A rate limit is an account-wide ceiling, not a
/// folder's, so the folders that did get through this pass got through by being ahead in the
/// queue; saying nothing until every one of them is refused would hide the pause for exactly as
/// long as it takes the account to stop syncing altogether.
///
/// Clearing needs only a pass that was not refused, unlike [`signin_expired`]: the notice states
/// a wait that has since either elapsed or been re-stated, it asks nothing of the user, and
/// leaving a stale one up says mail is not arriving when it is. An all-[`Reach::Busy`] pass
/// proves nothing either way and leaves it alone.
fn throttled(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_throttled = false;
    let mut any_verdict = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            Reach::Throttled => {
                any_throttled = true;
                any_verdict = true;
            }
            Reach::Reached | Reach::Expired | Reach::Unreachable => any_verdict = true,
            Reach::Busy => {}
        }
    }
    any_verdict.then_some(any_throttled)
}

#[cfg(test)]
#[path = "sync_account_tests.rs"]
mod reachability_tests;
