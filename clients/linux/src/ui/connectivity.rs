//! OS reachability, the core's connectivity snapshot, and the banners that render it.

use std::collections::HashSet;

use adw::prelude::*;
use gtk::gio::{self, prelude::NetworkMonitorExt};
use mailcal_bindings::{AccountProvider, AccountRow, ConnectivitySnapshot, Intent, MailcalApp};

use super::{AppInput, PrimaryView};
use crate::l10n;

/// The connectivity fields this client renders, resolved against the account list once per
/// connectivity signal rather than on every GTK render.
#[derive(Default)]
pub(super) struct ConnectivityState {
    pub(super) offline: bool,
    pub(super) unreachable_accounts: HashSet<String>,
    pub(super) calendar_reauth: Vec<NamedAccount>,
    pub(super) mail_reauth: Vec<NamedAccount>,
    pub(super) expired_signins: Vec<ExpiredSignIn>,
}

impl ConnectivityState {
    pub(super) fn pull(app: &MailcalApp, accounts: &[AccountRow]) -> Self {
        Self::from_snapshot(app.connectivity(), accounts, |id| {
            app.account_provider(id.to_owned())
        })
    }

    fn from_snapshot(
        snapshot: ConnectivitySnapshot,
        accounts: &[AccountRow],
        provider: impl Fn(&str) -> Option<AccountProvider>,
    ) -> Self {
        let expired_signins = snapshot
            .signin_expired_accounts
            .into_iter()
            .map(|id| ExpiredSignIn {
                id: id.clone(),
                email: account_email(accounts, &id),
                provider: provider(&id),
            })
            .collect();
        let named = |ids: Vec<String>| {
            ids.into_iter()
                .map(|id| NamedAccount {
                    email: account_email(accounts, &id),
                    id,
                })
                .collect()
        };
        Self {
            offline: snapshot.offline,
            unreachable_accounts: snapshot.unreachable_accounts.into_iter().collect(),
            calendar_reauth: named(snapshot.calendar_reauth_accounts),
            mail_reauth: named(snapshot.mail_reauth_accounts),
            expired_signins,
        }
    }

    pub(super) fn expired_resolution(&self) -> Option<ExpiredResolution> {
        self.expired_signins.first().map(ExpiredSignIn::resolution)
    }
}

/// An account a banner names, with the id its button acts on.
pub(super) struct NamedAccount {
    pub(super) id: String,
    pub(super) email: String,
}

fn names(accounts: &[NamedAccount]) -> String {
    accounts
        .iter()
        .map(|account| account.email.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn account_email(accounts: &[AccountRow], id: &str) -> String {
    accounts
        .iter()
        .find(|account| account.id == id)
        .map_or_else(|| id.to_owned(), |account| account.email.clone())
}

pub(super) struct ExpiredSignIn {
    pub(super) id: String,
    pub(super) email: String,
    provider: Option<AccountProvider>,
}

impl ExpiredSignIn {
    fn resolution(&self) -> ExpiredResolution {
        match self.provider.as_ref() {
            Some(AccountProvider::Microsoft | AccountProvider::Google) => {
                ExpiredResolution::SignInAgain(self.id.clone())
            }
            Some(AccountProvider::JmapOauth) => ExpiredResolution::JmapOauth(self.id.clone()),
            Some(AccountProvider::Password | AccountProvider::Jmap) | None => {
                ExpiredResolution::Settings
            }
        }
    }
}

pub(super) enum ExpiredResolution {
    /// A Microsoft or Google account, signed in again in place by its id.
    SignInAgain(String),
    JmapOauth(String),
    Settings,
}

/// Primes the snapshot and reachability before the first GTK render.
pub(super) fn at_launch(
    app: Option<&MailcalApp>,
    accounts: &[AccountRow],
    sender: &relm4::Sender<AppInput>,
) -> (ConnectivityState, gio::NetworkMonitor) {
    let state = app.map_or_else(ConnectivityState::default, |app| {
        ConnectivityState::pull(app, accounts)
    });
    let monitor = observe_network(sender);
    let reachable = monitor.is_network_available();
    if let Some(app) = app {
        app.dispatch(Intent::ReportNetworkReachable { reachable });
        // The cached snapshot is already ready to paint while offline. Coming online dispatches
        // its own catch-up refresh, so only a reachable launch needs this first one.
        if reachable {
            app.dispatch(Intent::RefreshMail);
        }
    }
    (state, monitor)
}

/// Connectivity strips. Account outages live in the folder pane instead.
pub(super) struct ConnectivityBanners {
    root: gtk::Box,
    pub(super) offline: adw::Banner,
    pub(super) expired: adw::Banner,
    pub(super) mail_reauth: adw::Banner,
    pub(super) calendar_reauth: adw::Banner,
}

impl ConnectivityBanners {
    pub(super) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let offline = adw::Banner::new("");
        offline.set_use_markup(false);
        root.append(&offline);
        let mail_reauth = adw::Banner::new("");
        mail_reauth.set_use_markup(false);
        mail_reauth.set_button_label(Some(l10n::mail_reauth_action()));
        let input = sender.clone();
        mail_reauth.connect_button_clicked(move |_| input.emit(AppInput::ResolveMailReauth));
        root.append(&mail_reauth);
        let calendar_reauth = adw::Banner::new("");
        calendar_reauth.set_use_markup(false);
        calendar_reauth.set_button_label(Some(l10n::calendar_reauth_action()));
        let input = sender.clone();
        calendar_reauth
            .connect_button_clicked(move |_| input.emit(AppInput::ResolveCalendarReauth));
        root.append(&calendar_reauth);
        let expired = adw::Banner::new("");
        expired.set_use_markup(false);
        let input = sender.clone();
        expired.connect_button_clicked(move |_| input.emit(AppInput::ResolveExpiredSignIn));
        root.append(&expired);
        Self {
            root,
            offline,
            expired,
            mail_reauth,
            calendar_reauth,
        }
    }

    pub(super) fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub(super) fn render(&self, state: &ConnectivityState, primary: PrimaryView) {
        self.offline.set_title(l10n::connectivity_offline_banner());
        self.offline.set_revealed(state.offline);

        self.mail_reauth
            .set_title(&l10n::mail_reauth_prompt(&names(&state.mail_reauth)));
        self.mail_reauth
            .set_revealed(primary == PrimaryView::Mail && !state.mail_reauth.is_empty());

        self.calendar_reauth
            .set_title(&l10n::calendar_reauth_prompt(&names(
                &state.calendar_reauth,
            )));
        self.calendar_reauth
            .set_revealed(primary == PrimaryView::Calendar && !state.calendar_reauth.is_empty());

        let names = state
            .expired_signins
            .iter()
            .map(|account| account.email.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let browser_flow = matches!(
            state.expired_resolution(),
            Some(ExpiredResolution::SignInAgain(_) | ExpiredResolution::JmapOauth(_))
        );
        let title = if browser_flow {
            l10n::signin_expired_prompt(&names)
        } else {
            l10n::signin_expired_prompt_settings(&names)
        };
        self.expired.set_title(&title);
        self.expired.set_button_label(Some(if browser_flow {
            l10n::signin_expired_action()
        } else {
            l10n::settings_title()
        }));
        self.expired.set_revealed(!state.expired_signins.is_empty());
    }
}

/// Retains GIO's default-network subscription and forwards changes onto the GLib/Relm loop.
pub(super) fn observe_network(sender: &relm4::Sender<AppInput>) -> gio::NetworkMonitor {
    let monitor = gio::NetworkMonitor::default();
    let input = sender.clone();
    monitor.connect_network_changed(move |_, reachable| {
        report_reachability(&input, reachable);
    });
    monitor
}

fn report_reachability(sender: &relm4::Sender<AppInput>, reachable: bool) {
    sender.emit(AppInput::NetworkReachabilityChanged(reachable));
}

#[cfg(test)]
#[path = "connectivity_tests.rs"]
pub(super) mod tests;
