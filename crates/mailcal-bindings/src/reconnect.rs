//! Bringing disconnected accounts up: what to *do* with a dial's outcome in a live app.
//!
//! Every interactive launch comes here. The boot returns with provider-less placeholders showing
//! cached mail, and this dials each account, joins the successes back into the app with live
//! providers, and turns each failure into the one thing a user can act on: an outage badge, or the
//! "sign in again" prompt when the server itself refused the stored credential.
//!
//! It is also the mid-session retry (a Refresh, a return to online), which is why the two are one
//! function: a recovered provider must heal into its full state; every folder, capabilities,
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
    SharedRegistry, account_registry::AccountDial, background::BackgroundManager, connection_log,
};

mod schedule;
mod uses;

pub(crate) use uses::reconnect_uses;

/// Dials every account in `plans`, at most `MAX_CONCURRENT_DIALS` at a time: on success joins it
/// back into the app (which syncs and clears its outage badge) and restarts its background sync
/// from the now-live settings; on an outage re-queues its id in `disconnected` so the next Refresh
/// or return to online retries it. A sign-in the server refused is not re-queued: dialling it
/// again presents the credential the server has just refused, so it waits for a replaced one
/// (`install_replacement`), which dials it itself.
///
/// Runs as one spawned task off the runtime, so a slow re-dial never blocks the host's dispatch.
///
/// Dials go three at a time, the same bound the headless boot uses, because "how much network may
/// we use at once" cannot sensibly differ by which path started the dial; each account's mail
/// catch-up starts as soon as its own dial lands, three at a time in a pool of their own, so a
/// long download never holds back a later account's dial. The calendar is fetched once every
/// dial has landed, beside the catch-ups; contacts follow everything. Both sync the accounts this
/// pass connected and no others, since an account already live has nothing new to fetch; what
/// they rebuild from the store covers every account.
pub(crate) async fn reconnect_all(
    app: Arc<App<Box<dyn Provider>>>,
    background: Arc<BackgroundManager>,
    registry: SharedRegistry,
    disconnected: Arc<Mutex<HashSet<String>>>,
    plans: Vec<(AccountId, AccountDial)>,
    display_zone: TimeZoneId,
) -> Vec<String> {
    // The accounts whose calendar or address-book server did not answer, for looking again
    // (`crate::dav_rediscovery`); one that has neither server stored is dropped there.
    let unanswered = Arc::new(Mutex::new(Vec::new()));
    let connected = Arc::new(Mutex::new(Vec::new()));
    let dial = |index: usize, (id, plan): (AccountId, AccountDial)| {
        let unanswered = Arc::clone(&unanswered);
        let connected = Arc::clone(&connected);
        let app = Arc::clone(&app);
        let registry = Arc::clone(&registry);
        let disconnected = Arc::clone(&disconnected);
        let display_zone = display_zone.clone();
        async move {
            // Captured before `run` consumes the plan. The label carries the address, which is fine
            // for an outage detail the user reads in the UI: the log lines below use the position.
            let account_type = plan.account_type();
            let label = plan.label();
            let uses_mail = plan
                .capabilities()
                .contains(mailcal_account::Capability::Mail);
            match plan.run(&id, display_zone).await {
                Ok(outcome) => {
                    // The user may have removed the account during the (slow) dial: its plan was
                    // snapshotted before the removal, so re-check the live registry before
                    // re-adding; otherwise a just-removed account would
                    // reappear with live providers.
                    if !registry.contains(id.as_str()) {
                        log::info!("reconnect: account[{index}] was removed mid-dial; discarding");
                        return None;
                    }
                    app.note_use_problems(&id, outcome.use_problems);
                    registry.learn_dav_starts(id.as_str(), &outcome.learned);
                    registry.note_dav_dialled(id.as_str(), std::time::Instant::now());
                    if let Some(detail) = &outcome.calendar_error {
                        log::warn!(
                            "reconnect: account[{index}] mail up but calendar failed: {detail}"
                        );
                    }
                    // Only a server that said its start is gone, or refused the sign-in, is worth
                    // finding again; one that did not answer is tried again as it is.
                    if outcome.rediscover {
                        unanswered
                            .lock()
                            .expect("unanswered mutex poisoned")
                            .push(id.as_str().to_owned());
                    }
                    // Reflect the calendar's re-consent state: if it connected, clear any prior
                    // prompt (e.g. the user just re-authenticated); if a scope-`403` withheld it,
                    // raise the "reconnect to enable calendar" prompt. A transient calendar
                    // failure leaves the prior state untouched.
                    if outcome.account.calendar_provider.is_none() {
                        if outcome.calendar_reauth_required {
                            app.note_calendar_reauth_required(&id);
                        }
                    } else {
                        app.clear_calendar_reauth_required(&id);
                    }
                    // Register the live providers WITHOUT syncing (`add_account_deferred`); the
                    // catch-up starts as soon as this returns, below. A plain `add_account` would
                    // show progress immediately over mail this account has already cached.
                    connection_log::log_account_connection_info(
                        &format!("account[{index}]"),
                        account_type,
                        &outcome.account,
                    );
                    app.add_account_deferred(outcome.account).await;
                    // A shared mailbox never passes through the step that asks an added account
                    // for its name, so it takes its provider's name here, once: a stored name
                    // ends the asking.
                    if registry.shared_parent(id.as_str()).is_some() {
                        app.needs_sender_name(&id).await;
                    }
                    connected
                        .lock()
                        .expect("connected mutex poisoned")
                        .push(id.clone());
                    Some(id)
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
                    // The account failed whole, so what its calendar or contacts last said is
                    // no longer the news; the account's own prompt is.
                    app.note_use_problems(&id, app.use_problems(&id).paused_only());
                    // Not re-queued: only a replaced credential can change the answer.
                    log::warn!("reconnect: account[{index}] sign-in refused by the server: {err}");
                    // Without mail, the server that refused is its calendar or address book
                    // server, which may have moved and left the old one refusing.
                    if !uses_mail {
                        unanswered
                            .lock()
                            .expect("unanswered mutex poisoned")
                            .push(id.as_str().to_owned());
                    }
                    None
                }
                Err(err) => {
                    // Badge the account unreachable with the account-labelled technical detail, so
                    // the connection-issues banner names it. At interactive boot every account is
                    // dialed through here, so a genuinely-unreachable one must badge just as a
                    // synchronous boot dial would, for a mid-session retry this refreshes the
                    // detail on the existing badge. Then re-queue it for the
                    // next attempt.
                    app.note_account_unreachable(&id, Some(format!("{label}: {err}")));
                    app.note_use_problems(&id, app.use_problems(&id).paused_only());
                    disconnected
                        .lock()
                        .expect("disconnected mutex poisoned")
                        .insert(id.as_str().to_owned());
                    log::warn!("reconnect: account[{index}] still unreachable: {err}");
                    // Looked for again only when a DAV server said its start is gone: an outage
                    // changes nothing, and a certificate that does not verify is asked about,
                    // never looked past.
                    if err.worth_rediscovering() {
                        unanswered
                            .lock()
                            .expect("unanswered mutex poisoned")
                            .push(id.as_str().to_owned());
                    }
                    None
                }
            }
        }
    };
    // This account's mail catch-up (the folder sync and the body prefetch after it), as soon as
    // its dial lands. Then its watches/poll restart from its now-live settings, so a server
    // that supports IMAP IDLE gets push again rather than the placeholder's poll timer; not
    // before, because IMAP watches sync once before IDLE, and starting them first can grab folder
    // scopes and make the catch-up look idle.
    let catch_up = |index: usize, id: AccountId| {
        let app = Arc::clone(&app);
        let background = Arc::clone(&background);
        async move {
            app.refresh_reconnected_account(&id).await;
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
    };
    // The calendar is fetched once every dial has landed, beside the mail catch-ups still running
    // rather than after them: a mailbox's body prefetch takes seconds, and a calendar waiting for
    // every account's would stay empty for all of them meanwhile. Here rather than at boot, since
    // an interactive launch returns with provider-less placeholders that a refresh reaches
    // nothing through; and on every launch, since the user may never open the tab
    // (`docs/calendar.md` §5).
    //
    // Called directly rather than dispatched: `Intent::RefreshCalendar` would record the calendar
    // as a feature the user used, on a launch where they may never open it.
    let calendar = async {
        let ids = connected.lock().expect("connected mutex poisoned").clone();
        app.refresh_calendar_of(&ids).await;
    };
    schedule::dial_then_catch_up(plans, dial, catch_up, calendar).await;
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
    let ids = std::mem::take(&mut *connected.lock().expect("connected mutex poisoned"));
    app.refresh_contacts_of(&ids).await;
    std::mem::take(&mut *unanswered.lock().expect("unanswered mutex poisoned"))
}

#[cfg(test)]
#[path = "reconnect/signin_refused_tests.rs"]
mod signin_refused_tests;

#[cfg(test)]
#[path = "reconnect/refresh_redial_tests.rs"]
mod refresh_redial_tests;
