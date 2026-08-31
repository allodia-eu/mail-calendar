//! Bringing disconnected accounts up: what to *do* with a dial's outcome in a live app.
//!
//! Every interactive launch comes here. The boot returns with provider-less placeholders showing
//! cached mail, and this dials each account, joins the successes back into the app with live
//! providers, and turns each failure into the one thing a user can act on: an outage badge, or the
//! "sign in again" prompt when the server itself refused the stored credential.
//!
//! It is also the mid-session retry (a Refresh, a return to online), which is why the two are one
//! function: a recovered provider must heal into its full state; role folders, capabilities,
//! calendar; rather than a degraded INBOX-only one, and that is the same work either way.
//!
//! **How** an account is dialed is not here. That is
//! [`AccountDial`](crate::account_registry::AccountDial), obtainable only from the registry, so
//! this module cannot open a socket for an account nobody registered. What is here is policy: the
//! order, the concurrency bound, and the meaning of each outcome.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use engine_api::{AccountId, Provider, TimeZoneId};
use mailcal_app::App;

use crate::{
    SharedRegistry,
    account_registry::{AccountDial, dial_all},
    background::BackgroundManager,
    connection_log,
};

/// Dials every account in `plans`, at most `MAX_CONCURRENT_DIALS` at a time: on success joins it
/// back into the app (which syncs and clears its outage badge) and restarts its background sync
/// from the now-live settings; on failure re-queues its id in `disconnected` so the next Refresh /
/// return-to-online retries it.
///
/// Runs as one spawned task off the runtime, so a slow re-dial never blocks the host's dispatch.
///
/// The bound is the change worth naming. This was a `for` loop with an `await` inside; strictly
/// sequential: so on a five-account device the fifth mailbox came alive only after four full
/// logins had finished one after another. It is now three at a time, the same bound the headless
/// boot uses, because "how much network may we use at once" cannot sensibly differ by which path
/// started the dial.
pub(crate) async fn reconnect_all(
    app: Arc<App<Box<dyn Provider>>>,
    background: Arc<BackgroundManager>,
    registry: SharedRegistry,
    disconnected: Arc<Mutex<HashSet<String>>>,
    plans: Vec<(AccountId, AccountDial)>,
    display_zone: TimeZoneId,
) {
    dial_all(plans, |index, (id, plan)| {
        let app = Arc::clone(&app);
        let background = Arc::clone(&background);
        let registry = Arc::clone(&registry);
        let disconnected = Arc::clone(&disconnected);
        let display_zone = display_zone.clone();
        async move {
            // Captured before `run` consumes the plan. The label carries the address, which is fine
            // for an outage detail the user reads in the UI: the log lines below use the position.
            let account_type = plan.account_type();
            let label = plan.label();
            match plan.run(&id, display_zone).await {
                Ok(outcome) => {
                    // The user may have removed the account during the (slow) dial: its plan was
                    // snapshotted before the removal, so re-check the live registry before
                    // re-adding; otherwise a just-removed account would
                    // reappear with live providers.
                    if !registry.contains(id.as_str()) {
                        log::info!("reconnect: account[{index}] was removed mid-dial; discarding");
                        return;
                    }
                    if let Some(detail) = &outcome.calendar_error {
                        log::warn!(
                            "reconnect: account[{index}] mail up but calendar failed: {detail}"
                        );
                    }
                    // Reflect the calendar's re-consent state: if it connected, clear any prior
                    // prompt (e.g. the user just re-authenticated); if a scope-`403` withheld it,
                    // raise the "reconnect to enable calendar" prompt. A transient calendar failure
                    // leaves the prior state untouched.
                    if outcome.account.calendar_providers.is_empty() {
                        if outcome.calendar_reauth_required {
                            app.note_calendar_reauth_required(&id);
                        }
                    } else {
                        app.clear_calendar_reauth_required(&id);
                    }
                    // Register the live providers WITHOUT syncing (`add_account_deferred`), then
                    // drive the catch-up refresh before restarting the account's watches/poll. IMAP
                    // watches sync once before IDLE; starting them first can grab folder scopes and
                    // make this resumed catch-up look idle after an app restart. A plain
                    // `add_account` would show progress immediately over mail this account has
                    // already cached.
                    connection_log::log_account_connection_info(
                        &format!("account[{index}]"),
                        account_type,
                        &outcome.account,
                    );
                    app.add_account_deferred(outcome.account).await;
                    app.refresh_reconnected_account(&id).await;
                    // Restart this account's watches/poll from its now-live (real-provider)
                    // settings, so a server that supports IMAP IDLE gets push again: not just the
                    // placeholder's poll timer.
                    let snapshot = app.sync_settings().await;
                    if let Some(row) = snapshot
                        .accounts
                        .iter()
                        .find(|row| row.account_id == id.as_str())
                    {
                        background.apply(id.as_str(), Some(row));
                    }
                    log::info!("reconnect: account[{index}] reconnected");
                }
                Err(err) if err.signin_expired() => {
                    // The server answered; it refused the stored credential, whichever kind the
                    // account holds (`docs/provider-oauth.md` rule 12). Raise the reconnect prompt
                    // and deliberately do NOT badge the account unreachable: "can't reach this
                    // account's server" would be a lie, and it points the user at waiting rather
                    // than at the sign-in or the Settings field that is the only remedy. This is
                    // the path a refused credential actually takes on every client: the interactive
                    // app dials nothing synchronously, so it surfaces here at boot, never through
                    // the sync-pass classifier.
                    app.note_signin_expired(&id);
                    // Still re-queue it, so re-authenticating (or the next Refresh) re-dials it.
                    disconnected
                        .lock()
                        .expect("disconnected mutex poisoned")
                        .insert(id.as_str().to_owned());
                    log::warn!("reconnect: account[{index}] sign-in refused by the server: {err}");
                }
                Err(err) => {
                    // Badge the account unreachable with the account-labelled technical detail, so
                    // the connection-issues banner names it. At interactive boot every account is
                    // dialed through here, so a genuinely-unreachable one must badge just as a
                    // synchronous boot dial would, for a mid-session retry this refreshes the
                    // detail on the existing badge. Then re-queue it for the
                    // next attempt.
                    app.note_account_unreachable(&id, Some(format!("{label}: {err}")));
                    disconnected
                        .lock()
                        .expect("disconnected mutex poisoned")
                        .insert(id.as_str().to_owned());
                    log::warn!("reconnect: account[{index}] still unreachable: {err}");
                }
            }
        }
    })
    .await;
    // Every account that was going to connect now has, so this is the first moment the calendar
    // *can* be fetched. It has to happen here rather than at boot: an interactive launch returns
    // with provider-less placeholders, and a refresh against those reaches nothing and files
    // nothing, which is how the calendar came to be filled by only one thing, the user opening
    // the tab. A user who never opened it therefore had no diary at all, on any launch, offline
    // or on (`docs/calendar.md` §5).
    //
    // Called directly rather than dispatched: `Intent::RefreshCalendar` would record the calendar
    // as a feature the user used, on a launch where they may never open it.
    app.refresh_calendar_in_background().await;
    // Contacts have exactly the calendar's problem above, and it went unnoticed for the same
    // reason: nothing but the user opening the tab ever filled them. It stopped being
    // invisible when a sender's face started coming from a contact card: a launch where the
    // user never opened Contacts showed no faces at all. Measured on macOS against the
    // harness: the boot-time refresh ran at `…58.497` and found "no contact sources bound",
    // while the source bound at `…59.352`.
    //
    // After the mail and calendar catch-up, not before: this is background work behind an
    // already-drawn list, and a cold or offline boot must still show everything cached
    // without waiting on any of it.
    app.refresh_contacts().await;
}
