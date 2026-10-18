//! OS reachability, the core's connectivity snapshot, and the banners that render it.

use std::{cell::RefCell, collections::HashSet};

use adw::prelude::*;
use gtk::gio::{self, prelude::NetworkMonitorExt};
use mailcal_bindings::{
    AccountCapability, AccountProvider, AccountRow, ConnectivitySnapshot, Intent, MailcalApp,
    UseOutage,
};

use super::{
    AppInput, PrimaryView,
    account_settings::AccountsInput,
    settings::{entry_name, outage_line},
};
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
    pub(super) calendar_outages: Vec<OutageLine>,
    pub(super) contacts_outages: Vec<OutageLine>,
}

impl ConnectivityState {
    pub(super) fn pull(app: &MailcalApp, accounts: &[AccountRow]) -> Self {
        // An account without mail is in none of the mailbox's rows, so an outage is named from
        // the snapshot that lists every account, read only when there is one to name. A banner
        // asking an account to sign in again names it apart from another with its address.
        let everyone = std::cell::OnceCell::new();
        let entry = |id: &str| {
            everyone
                .get_or_init(|| app.accounts_snapshot())
                .accounts
                .iter()
                .find(|entry| entry.id == id)
                .cloned()
        };
        Self::from_snapshot(
            app.connectivity(),
            accounts,
            |id| entry(id).map(|entry| entry.address),
            |id| entry(id).map(|entry| entry_name(&entry)),
            |id| app.account_provider(id.to_owned()),
        )
    }

    fn from_snapshot(
        snapshot: ConnectivitySnapshot,
        accounts: &[AccountRow],
        address: impl Fn(&str) -> Option<String>,
        name: impl Fn(&str) -> Option<String>,
        provider: impl Fn(&str) -> Option<AccountProvider>,
    ) -> Self {
        let named_by = |id: &str| name(id).unwrap_or_else(|| account_email(accounts, id));
        let expired_signins = snapshot
            .signin_expired_accounts
            .into_iter()
            .map(|id| ExpiredSignIn {
                email: named_by(&id),
                provider: provider(&id),
                id,
            })
            .collect();
        let named = |ids: Vec<String>| {
            ids.into_iter()
                .map(|id| NamedAccount {
                    email: named_by(&id),
                    id,
                })
                .collect()
        };
        let outages = |capability, outages: Vec<UseOutage>| {
            outages
                .into_iter()
                .map(|outage| {
                    let address = address(&outage.account_id)
                        .unwrap_or_else(|| account_email(accounts, &outage.account_id));
                    OutageLine {
                        text: outage_line(capability, outage.problem, &address),
                        account: outage.account_id,
                    }
                })
                .collect()
        };
        Self {
            calendar_outages: outages(AccountCapability::Calendar, snapshot.calendar_outages),
            contacts_outages: outages(AccountCapability::Contacts, snapshot.contacts_outages),
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

/// One account's calendar or address book that did not connect: the line said above the week or
/// the list, and the account whose page its button opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutageLine {
    pub(super) account: String,
    pub(super) text: String,
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
            Some(AccountProvider::ImapOauth) => ExpiredResolution::ImapOauth(self.id.clone()),
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
    /// An IMAP account that signs in through its provider, signed in again in place from its
    /// stored grant. It has no password to ask for.
    ImapOauth(String),
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
    pub(super) calendar_outages: OutageBanners,
    pub(super) contacts_outages: OutageBanners,
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
        let calendar_outages = OutageBanners::new(sender);
        root.append(&calendar_outages.root);
        let contacts_outages = OutageBanners::new(sender);
        root.append(&contacts_outages.root);
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
            calendar_outages,
            contacts_outages,
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
        self.calendar_outages
            .render(&state.calendar_outages, primary == PrimaryView::Calendar);
        self.contacts_outages
            .render(&state.contacts_outages, primary == PrimaryView::Contacts);

        let names = state
            .expired_signins
            .iter()
            .map(|account| account.email.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let browser_flow = matches!(
            state.expired_resolution(),
            Some(
                ExpiredResolution::SignInAgain(_)
                    | ExpiredResolution::JmapOauth(_)
                    | ExpiredResolution::ImapOauth(_)
            )
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

/// One banner per account whose calendar or address book did not connect, in the snapshot's
/// order. The banners are rebuilt only when the lines change, so a redraw does not replay a reveal.
pub(super) struct OutageBanners {
    pub(super) root: gtk::Box,
    drawn: RefCell<Vec<OutageLine>>,
    sender: relm4::Sender<AppInput>,
}

impl OutageBanners {
    fn new(sender: &relm4::Sender<AppInput>) -> Self {
        Self {
            root: gtk::Box::new(gtk::Orientation::Vertical, 0),
            drawn: RefCell::default(),
            sender: sender.clone(),
        }
    }

    fn render(&self, lines: &[OutageLine], shown: bool) {
        self.root.set_visible(shown && !lines.is_empty());
        if self.drawn.borrow().as_slice() == lines {
            return;
        }
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        for line in lines {
            let banner = adw::Banner::new("");
            banner.set_use_markup(false);
            banner.set_title(&line.text);
            banner.set_button_label(Some(l10n::use_outage_open_account()));
            let input = self.sender.clone();
            let account = line.account.clone();
            banner.connect_button_clicked(move |_| {
                input.emit(AppInput::Accounts(AccountsInput::Open(account.clone())));
            });
            banner.set_revealed(true);
            self.root.append(&banner);
        }
        lines.clone_into(&mut self.drawn.borrow_mut());
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
