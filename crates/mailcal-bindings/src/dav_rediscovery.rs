//! Finding an account's calendar and address-book servers again when the stored ones stop
//! answering (`docs/account-autodetect.md` rule 14).
//!
//! RFC 6764 §6 asks a client to keep the servers that worked and to bootstrap again when they
//! fail. A reconnect that could not reach an account's calendar hands the account here; the
//! lookup setup ran is run again for its address, and a server it finds is followed only under
//! the registrable domain of the one it replaces or of the address (RFC 6764 §8: DNS is not
//! authenticated, so a stored password never follows an answer anywhere else). The move itself
//! is [`MailcalApp::update_account_endpoints`]: connect before anything is stored, and what the
//! device held of a use whose server moved host is synced again from the new one.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

use mailcal_account::account_log_handle;

use crate::{AccountEndpoints, MailcalApp, MxResolver, autodetect::CallbackMxResolver};

/// How long after one attempt an account is looked for again: a server that is down rather than
/// moved answers the same lookup the same way, and asking every retry would only cost requests.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);

/// What the app keeps for looking again: the host's DNS, each account's last attempt, and the
/// app itself, so a reconnect running on the runtime can hand an account over once it is done.
#[derive(Default)]
pub(crate) struct Rediscovery {
    resolver: Mutex<Option<Arc<dyn mailcal_autodetect::MxResolver>>>,
    attempts: Mutex<HashMap<String, Instant>>,
    app: OnceLock<Weak<MailcalApp>>,
}

impl Rediscovery {
    /// Lets a reconnect reach `app` once it is done, without keeping it alive.
    pub(crate) fn adopt(&self, app: &Arc<MailcalApp>) {
        let _ = self.app.set(Arc::downgrade(app));
    }

    /// The app, while it is still running.
    pub(crate) fn app(&self) -> Option<Arc<MailcalApp>> {
        self.app.get().and_then(Weak::upgrade)
    }

    /// Whether `account_id` is due a look, recording the attempt when it is.
    fn due(&self, account_id: &str, now: Instant) -> bool {
        let mut attempts = self.attempts.lock().expect("attempts mutex poisoned");
        let due = attempts
            .get(account_id)
            .is_none_or(|last| now.duration_since(*last) >= RETRY_AFTER);
        if due {
            attempts.insert(account_id.to_owned(), now);
        }
        due
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Hands the core the host's DNS, the resolver setup's detection takes, so it can look again
    /// for a calendar or address-book server when the stored one stops answering. Call it right
    /// after the app is built; until it is called the core does not look again.
    pub fn set_dns_resolver(&self, resolver: Box<dyn MxResolver>) {
        *self
            .rediscovery
            .resolver
            .lock()
            .expect("resolver mutex poisoned") = Some(Arc::new(CallbackMxResolver(resolver)));
    }
}

impl MailcalApp {
    /// Looks again for `account_id`'s calendar and address-book servers and moves the account
    /// onto what discovery finds now, when that differs and may be followed. At most once an
    /// hour per account; a no-op for an account with neither server stored.
    ///
    /// **Blocking** (discovery and connect); call it off the runtime's threads.
    pub(crate) fn rediscover_servers(&self, account_id: &str) {
        let Some(entry) = self
            .accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == account_id)
        else {
            return;
        };
        let Some(stored) = entry
            .endpoints
            .filter(|endpoints| endpoints.caldav_url.is_some() || endpoints.carddav_url.is_some())
        else {
            return;
        };
        let handle = account_log_handle(account_id);
        // Before the host has handed its DNS over, which it does right after building the app,
        // the account is left for the next reconnect rather than looked for without SRV and then
        // left alone for an hour.
        let Some(resolver) = self
            .rediscovery
            .resolver
            .lock()
            .expect("resolver mutex poisoned")
            .clone()
        else {
            log::debug!("rediscovery: [{handle}] no DNS yet; left for the next reconnect");
            return;
        };
        if !self.rediscovery.due(account_id, Instant::now()) {
            return;
        }
        let host = stored
            .caldav_url
            .as_deref()
            .or(stored.carddav_url.as_deref())
            .and_then(|url| url::Url::parse(url).ok())
            .and_then(|url| url.host_str().map(str::to_owned));
        let address = entry.address.clone();
        let found = self.runtime.block_on(|| async move {
            mailcal_autodetect::discover_dav(
                &address,
                host.as_deref(),
                Some(resolver),
                &mailcal_autodetect::DetectConfig::default(),
            )
            .await
        });
        let found = match found {
            Ok(found) => found,
            Err(error) => {
                log::warn!("rediscovery: [{handle}] could not look again: {error}");
                return;
            }
        };
        let Some(moved) = moved_endpoints(&stored, &found, &entry.address, &handle) else {
            log::info!("rediscovery: [{handle}] discovery names the same servers; kept");
            return;
        };
        match self.update_account_endpoints(account_id.to_owned(), moved) {
            Ok(()) => {
                log::info!("rediscovery: [{handle}] moved to the servers discovery names now")
            }
            Err(error) => {
                log::warn!("rediscovery: [{handle}] the servers found did not connect: {error}")
            }
        }
    }
}

/// `stored` with each server discovery found somewhere new put in place of the old one, when it
/// may be followed; `None` when nothing moved that may be.
fn moved_endpoints(
    stored: &AccountEndpoints,
    found: &mailcal_autodetect::DetectedDav,
    address: &str,
    handle: &str,
) -> Option<AccountEndpoints> {
    let follow = |stored: &Option<String>, found: &Option<String>, service: &str| {
        let (stored, found) = (stored.as_deref()?, found.as_deref()?);
        if stored == found {
            return None;
        }
        if mailcal_autodetect::may_follow(found, stored, address) {
            Some(found.to_owned())
        } else {
            log::info!(
                "rediscovery: [{handle}] the {service} server found is not the provider's; not \
                 followed"
            );
            None
        }
    };
    let caldav = follow(&stored.caldav_url, &found.caldav_url, "calendar");
    let carddav = follow(&stored.carddav_url, &found.carddav_url, "address-book");
    (caldav.is_some() || carddav.is_some()).then(|| AccountEndpoints {
        caldav_url: caldav.or_else(|| stored.caldav_url.clone()),
        carddav_url: carddav.or_else(|| stored.carddav_url.clone()),
        password: None,
        ..stored.clone()
    })
}

#[cfg(test)]
#[path = "dav_rediscovery_tests.rs"]
mod tests;
