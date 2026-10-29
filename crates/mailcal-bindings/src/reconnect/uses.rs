//! Dialling again a calendar or address book that failed beside a working mailbox.
//!
//! The mailbox is connected, so nothing about it is dialled, synced or watched again: each
//! account's calendar and contacts are opened on their own
//! ([`AccountDial::run_uses`](crate::account_registry::AccountDial::run_uses)) and installed
//! around the mail providers it already has. Signing in to the mailbox again on every retry is
//! what makes a provider limit the account's sign-ins, for a failure that was never the
//! mailbox's.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Provider, TimeZoneId};
use mailcal_app::{App, UsesReplaced};

use crate::{
    SharedRegistry,
    account_registry::{AccountDial, dial_all},
};

/// Dials the calendar and contacts of every account in `plans`, at most `MAX_CONCURRENT_DIALS`
/// at a time, installs what connected, and then refreshes the calendar and contacts of the
/// accounts it reconnected. What each dial found replaces what the one before it said.
///
/// Returns the accounts whose calendar still did not answer, for looking again
/// (`crate::dav_rediscovery`).
pub(crate) async fn reconnect_uses(
    app: Arc<App<Box<dyn Provider>>>,
    registry: SharedRegistry,
    plans: Vec<(AccountId, AccountDial)>,
    display_zone: TimeZoneId,
) -> Vec<String> {
    let unanswered = Arc::new(Mutex::new(Vec::new()));
    let reconnected = Arc::new(Mutex::new(Vec::new()));
    dial_all(plans, |index, (id, plan): (AccountId, AccountDial)| {
        let app = Arc::clone(&app);
        let registry = Arc::clone(&registry);
        let unanswered = Arc::clone(&unanswered);
        let reconnected = Arc::clone(&reconnected);
        let display_zone = display_zone.clone();
        async move {
            match plan.run_uses(&id, display_zone).await {
                // Removed during the dial: its plan predates the removal.
                Ok(_) if !registry.contains(id.as_str()) => {}
                Ok(outcome) => {
                    registry.learn_dav_starts(id.as_str(), &outcome.learned);
                    if outcome.rediscover {
                        unanswered
                            .lock()
                            .expect("unanswered mutex poisoned")
                            .push(id.as_str().to_owned());
                    }
                    let problems = outcome.use_problems;
                    let unanswered = outcome.unanswered;
                    let reauth = outcome.calendar_reauth_required;
                    let calendar = outcome.account.calendar_provider.is_some();
                    match app.replace_uses(outcome.account, unanswered).await {
                        UsesReplaced::Replaced => {}
                        // Not noted as dialled, so the next retry dials it again.
                        UsesReplaced::Kept => {
                            log::info!(
                                "reconnect: account[{index}] calendar or contacts did not answer \
                                 the redial; keeping the connection it has"
                            );
                            return;
                        }
                        UsesReplaced::Gone => return,
                    }
                    app.note_use_problems(&id, problems);
                    registry.note_dav_dialled(id.as_str(), std::time::Instant::now());
                    if calendar {
                        app.clear_calendar_reauth_required(&id);
                    } else if reauth {
                        app.note_calendar_reauth_required(&id);
                    }
                    log::info!(
                        "reconnect: account[{index}] calendar and contacts dialled again; \
                         calendar {:?}, contacts {:?}",
                        problems.calendar,
                        problems.contacts,
                    );
                    reconnected
                        .lock()
                        .expect("reconnected mutex poisoned")
                        .push(id);
                }
                // What its calendar and contacts last failed with still stands.
                Err(err) => {
                    log::warn!(
                        "reconnect: account[{index}] calendar and contacts still failed: {err}"
                    );
                }
            }
        }
    })
    .await;
    let ids = std::mem::take(&mut *reconnected.lock().expect("reconnected mutex poisoned"));
    if !ids.is_empty() {
        app.refresh_calendar_of(&ids).await;
        app.refresh_contacts_of(&ids).await;
    }
    std::mem::take(&mut *unanswered.lock().expect("unanswered mutex poisoned"))
}
