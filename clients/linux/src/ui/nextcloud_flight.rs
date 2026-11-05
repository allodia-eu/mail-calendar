//! The browser half of a Nextcloud sign-in, which every surface offering it shares
//! (`docs/nextcloud-sign-in.md` rules 5 and 6). The core starts the sign-in, the browser opens on
//! the server's own login page, and a worker asks the core whether the person has finished: every
//! poll interval, and at once when one of this app's windows is active again. There is no
//! redirect back, so the worker is all the waiting there is.
//!
//! What a surface does with the grant is its own: each completion is a different core call. What
//! they share is that the server hands the grant out once, so a grant that arrives for an attempt
//! nobody is waiting on any more is withdrawn rather than dropped.

use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use mailcal_bindings::{MailcalApp, MailcalError, NextcloudSignInPoll, NextcloudSignInStart};

use crate::l10n;

/// A granted sign-in, held until it becomes an account. Never printed: it carries the app
/// password.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Grant(pub(crate) String);

impl std::fmt::Debug for Grant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Grant(..)")
    }
}

/// What a flight reports, in order: `Opened` at most once, then one `Ended`.
#[derive(Debug)]
pub(crate) enum Flight {
    /// The core started the sign-in; the browser opens on this page.
    Opened(String),
    Ended(FlightEnd),
}

#[derive(Debug)]
pub(crate) enum FlightEnd {
    Granted(Grant),
    /// Twenty minutes passed and the server let the sign-in go.
    Expired,
    /// What to say about why the sign-in did not start, or ended without a grant.
    Failed(String),
    /// The attempt was retired. A grant that arrived after that has been withdrawn.
    Cancelled,
}

impl FlightEnd {
    /// What a surface says about an end without a grant: `None` for a cancelled one, which says
    /// nothing.
    pub(super) fn said(&self) -> Option<String> {
        match self {
            Self::Granted(_) | Self::Cancelled => None,
            Self::Expired => Some(l10n::account_signin_timeout().to_owned()),
            Self::Failed(said) => Some(said.clone()),
        }
    }
}

thread_local! {
    /// The workers waiting on a person in the browser, each woken by a send. Main thread only:
    /// a window's activation is where the wake comes from.
    static WAITING: RefCell<Vec<mpsc::Sender<()>>> = const { RefCell::new(Vec::new()) };
}

/// One of this app's windows is active again, which is when somebody who just granted access in
/// the browser is back: every flight asks now rather than at its next interval.
pub(super) fn returned() {
    WAITING.with_borrow_mut(|waiting| {
        waiting.retain(|wake| wake.send(()).is_ok());
        if !waiting.is_empty() {
            log::info!("nextcloud: a window is active again; asking now");
        }
    });
}

/// Connects `window` to [`returned`], for each window a person comes back to.
pub(super) fn wake_on_return(window: &impl gtk::prelude::IsA<gtk::Window>) {
    use gtk::prelude::GtkWindowExt;
    window.connect_is_active_notify(|window| {
        if window.is_active() {
            returned();
        }
    });
}

/// What a refusal to start, or to poll, says: the core's mapping, with a failed connect read as
/// the sign-in having failed, since the server's own words are for a developer.
pub(super) fn said(error: &MailcalError) -> String {
    match error {
        MailcalError::Connect(_) => l10n::setup_imap_signin_failed().to_owned(),
        other => super::core_error::said(other),
    }
}

/// Opens the browser on `login_url`, calling `on_error` on the main thread when it did not open.
pub(super) fn open_browser(login_url: &str, on_error: impl FnOnce() + 'static) {
    super::oauth_loopback::launch_browser(login_url, on_error);
}

/// Flies one sign-in on a worker: `begin` is the surface's own start, and `report` hears the
/// flight's events from the worker, in order. The worker stops at the next poll once `cancel`
/// is raised. Called on the main thread, which is where the wake is registered.
pub(super) fn fly(
    app: Arc<MailcalApp>,
    cancel: Arc<AtomicBool>,
    begin: impl FnOnce(&MailcalApp) -> Result<NextcloudSignInStart, MailcalError> + Send + 'static,
    report: impl Fn(Flight) + Send + 'static,
) {
    let (wake, woken) = mpsc::channel();
    WAITING.with_borrow_mut(|waiting| waiting.push(wake));
    let interval = Duration::from_millis(mailcal_bindings::nextcloud_poll_interval_ms());
    std::thread::spawn(move || {
        let end = match begin(&app) {
            Ok(start) => {
                report(Flight::Opened(start.login_url));
                wait(&cancel, &start.pending, interval, &woken, |pending| {
                    app.poll_nextcloud_sign_in(pending.to_owned())
                })
            }
            Err(error) => FlightEnd::Failed(said(&error)),
        };
        if let (FlightEnd::Granted(grant), true) = (&end, cancel.load(Ordering::Acquire)) {
            app.abandon_nextcloud_sign_in(grant.0.clone());
            return report(Flight::Ended(FlightEnd::Cancelled));
        }
        report(Flight::Ended(end));
    });
}

/// Polls `pending` until the sign-in ends or `cancel` is raised, waiting `interval` between
/// asks, or less when `woken` hears a window come back.
fn wait(
    cancel: &AtomicBool,
    pending: &str,
    interval: Duration,
    woken: &mpsc::Receiver<()>,
    poll: impl Fn(&str) -> Result<NextcloudSignInPoll, MailcalError>,
) -> FlightEnd {
    loop {
        if cancel.load(Ordering::Acquire) {
            return FlightEnd::Cancelled;
        }
        match poll(pending) {
            Ok(NextcloudSignInPoll::Waiting) => {}
            Ok(NextcloudSignInPoll::Expired) => return FlightEnd::Expired,
            Ok(NextcloudSignInPoll::Granted { grant }) => return FlightEnd::Granted(Grant(grant)),
            Err(error) => return FlightEnd::Failed(said(&error)),
        }
        // A wake and a timeout both mean "ask again". A wake that can never come still waits
        // the interval out, rather than asking the server as fast as it answers.
        if let Err(mpsc::RecvTimeoutError::Disconnected) = woken.recv_timeout(interval) {
            std::thread::sleep(interval);
        }
    }
}

#[cfg(test)]
#[path = "nextcloud_flight_tests.rs"]
mod tests;
