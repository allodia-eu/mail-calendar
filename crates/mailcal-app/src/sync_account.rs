//! Per-account sync of a single account's folders, plus reachability classification.
//!
//! Split out of `sync.rs` to keep each file under the size limit. [`sync_account_providers`]
//! hands the account's providers to the engine, which syncs the folder list once and fans the
//! folders out itself, and turns the per-scope report back into the two product answers only
//! this layer can give: whether the account reached its server ([`Reach`], for the outage badge)
//! and whether its sign-in was refused. It also writes the pass to the diagnostic log.

use engine_api::{Engine, MailSyncReport, Provider, StreamTuning, SyncError, SyncObserver};

use crate::{Account, connectivity::is_signin_expired};

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
    /// The op was skipped because a concurrent sync held the scope ([`ApiError::Busy`]); no
    /// bearing on reachability.
    Busy,
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
            Reach::Reached => any_reached = true,
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
            Reach::Unreachable | Reach::Busy => {}
        }
    }
    match (any_reached, any_expired) {
        (false, true) => Some(true),
        (true, false) => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod reachability_tests {
    use super::{Reach, reachability, signin_expired};

    #[test]
    fn a_refused_credential_raises_the_prompt_and_is_not_an_outage() {
        // A dead OAuth grant fails every op. The account is *reachable* (the server answered and
        // said no), so the outage badge stays off and the reconnect prompt carries the story.
        let expired = || [Reach::Expired, Reach::Expired].into_iter();
        assert_eq!(reachability(Reach::Expired, expired()), Some(true));
        assert_eq!(signin_expired(Reach::Expired, expired()), Some(true));
    }

    #[test]
    fn a_refused_credential_outranks_a_transport_failure() {
        // A blip on one folder alongside a dead grant must not downgrade the account to a
        // generic outage, that would hide the one message naming the actual remedy.
        assert_eq!(
            reachability(Reach::Expired, [Reach::Unreachable].into_iter()),
            Some(true),
        );
        assert_eq!(
            signin_expired(Reach::Unreachable, [Reach::Expired].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn a_refusal_beside_a_success_neither_raises_nor_retracts() {
        // One credential serves every scope on an account, so a scope that authenticated in this
        // pass proves the stored credential is still accepted: the refusal is the server's, and
        // must not cost the user a sign-in they do not need.
        assert_eq!(
            signin_expired(Reach::Reached, [Reach::Expired].into_iter()),
            None,
        );
        assert_eq!(
            signin_expired(Reach::Expired, [Reach::Reached].into_iter()),
            None,
        );
        // Nor is a mixed pass evidence to retract a prompt already standing: it proves nothing
        // about the credential the user was asked to renew.
        //
        // A concurrent-sync skip is not a success, so a refusal alongside one still raises; we
        // cannot see whether the sync holding that scope is succeeding.
        assert_eq!(
            signin_expired(Reach::Busy, [Reach::Expired].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn only_a_success_retracts_the_prompt() {
        // Signing in again (or the grant simply working) clears it…
        assert_eq!(
            signin_expired(Reach::Reached, [Reach::Reached].into_iter()),
            Some(false),
        );
        // …but a transport failure or a concurrent-sync skip is no evidence the credential
        // works, so neither may retract a prompt the user still has to act on.
        assert_eq!(signin_expired(Reach::Unreachable, [].into_iter()), None);
        assert_eq!(signin_expired(Reach::Busy, [Reach::Busy].into_iter()), None);
        // A pass with nothing to say about credentials leaves the prompt alone even when it
        // does have something to say about reachability.
        assert_eq!(
            reachability(Reach::Unreachable, [].into_iter()),
            Some(false),
        );
    }

    #[test]
    fn any_success_means_reachable_even_with_a_failed_folder() {
        // The list reached; a folder failing doesn't make the whole account unreachable.
        assert_eq!(
            reachability(Reach::Reached, [Reach::Unreachable].into_iter()),
            Some(true),
        );
        // Or the list failed but a folder reached.
        assert_eq!(
            reachability(Reach::Unreachable, [Reach::Reached].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn every_op_failing_reads_unreachable() {
        assert_eq!(
            reachability(
                Reach::Unreachable,
                [Reach::Unreachable, Reach::Unreachable].into_iter(),
            ),
            Some(false),
        );
    }

    #[test]
    fn all_busy_is_indeterminate() {
        // A concurrent sync held every scope: no reachability signal, so leave the badge
        // as-is (a concurrent poll + refresh must not falsely mark an account unreachable).
        assert_eq!(reachability(Reach::Busy, [Reach::Busy].into_iter()), None);
    }

    #[test]
    fn busy_never_overrides_a_real_signal() {
        // Busy is ignored, so a real failure alongside busy still reads unreachable…
        assert_eq!(
            reachability(Reach::Busy, [Reach::Unreachable].into_iter()),
            Some(false),
        );
        // …and a success alongside busy still reads reachable.
        assert_eq!(
            reachability(Reach::Busy, [Reach::Reached].into_iter()),
            Some(true),
        );
    }
}
