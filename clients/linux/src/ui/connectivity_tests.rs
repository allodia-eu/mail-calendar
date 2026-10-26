//! Regressions for the Linux host's connectivity projection and reachability bridge.

use adw::prelude::*;
use mailcal_bindings::{
    AccountCapability, AccountProvider, AccountRow, ConnectivitySnapshot, ServerChange, UseOutage,
    UseProblem,
};

use super::{
    ConnectivityBanners, ConnectivityState, ExpiredResolution, OutageBanners, OutageLine,
    report_reachability,
};
use crate::ui::{
    AppInput, PrimaryView, account_settings::AccountsInput, server_change::Review,
    settings::outage_line,
};

fn account(id: &str, email: &str) -> AccountRow {
    AccountRow {
        id: id.to_owned(),
        email: email.to_owned(),
        name: String::new(),
        expanded: true,
    }
}

fn snapshot() -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        offline: true,
        unreachable_accounts: vec!["outage".to_owned()],
        calendar_reauth_accounts: vec!["calendar".to_owned()],
        mail_reauth_accounts: vec!["mail".to_owned()],
        signin_expired_accounts: vec!["expired".to_owned(), "removed".to_owned()],
        calendar_outages: vec![
            outage("cal-only", UseProblem::NeedsOwnSignIn),
            outage("mail", UseProblem::Unreachable),
        ],
        contacts_outages: vec![outage("mail", UseProblem::SignInRefused)],
    }
}

fn outage(account: &str, problem: UseProblem) -> UseOutage {
    UseOutage {
        account_id: account.to_owned(),
        problem,
    }
}

/// The accounts snapshot's addresses: it lists an account without mail, which the mailbox's rows
/// do not.
fn address(id: &str) -> Option<String> {
    (id == "cal-only").then(|| "calendar-only@example.test".to_owned())
}

fn no_address(_: &str) -> Option<String> {
    None
}

fn no_change(_: &str) -> Option<ServerChange> {
    None
}

fn moved() -> ServerChange {
    ServerChange {
        previous_site: "icloud.com".to_owned(),
        proposed_site: "example.net".to_owned(),
        calendar: true,
        contacts: false,
    }
}

fn texts(banners: &OutageBanners) -> Vec<String> {
    let mut texts = Vec::new();
    let mut child = banners.root.first_child();
    while let Some(widget) = child {
        let banner = widget.downcast_ref::<adw::Banner>().expect("only banners");
        assert!(banner.is_revealed());
        assert_eq!(
            banner.button_label().as_deref(),
            Some(crate::l10n::use_outage_open_account())
        );
        texts.push(banner.title().to_string());
        child = widget.next_sibling();
    }
    texts
}

#[test]
fn one_snapshot_projects_the_offline_outage_and_expired_signin_states() {
    let state = ConnectivityState::from_snapshot(
        snapshot(),
        &[
            account("outage", "outage@example.test"),
            account("expired", "expired@example.test"),
            account("calendar", "calendar@example.test"),
            account("mail", "mail@example.test"),
        ],
        address,
        no_address,
        |id| (id == "expired").then_some(AccountProvider::Google),
        no_change,
    );

    assert!(state.offline);
    assert!(state.unreachable_accounts.contains("outage"));
    let named = |accounts: &[super::NamedAccount]| {
        accounts
            .iter()
            .map(|account| (account.id.clone(), account.email.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        named(&state.calendar_reauth),
        [("calendar".to_owned(), "calendar@example.test".to_owned())]
    );
    assert_eq!(
        named(&state.mail_reauth),
        [("mail".to_owned(), "mail@example.test".to_owned())]
    );
    assert_eq!(state.expired_signins.len(), 2);
    assert_eq!(state.expired_signins[0].id, "expired");
    assert_eq!(state.expired_signins[0].email, "expired@example.test");
    assert!(matches!(
        state.expired_signins[0].resolution(),
        ExpiredResolution::SignInAgain(account) if account == "expired"
    ));
    assert_eq!(
        state.expired_signins[1].email, "removed",
        "an account removed between snapshots still gets an honest prompt"
    );
    assert!(matches!(
        state.expired_signins[1].resolution(),
        ExpiredResolution::Settings
    ));
}

#[test]
fn each_outage_is_one_line_in_the_snapshots_order_naming_the_accounts_address() {
    let state = ConnectivityState::from_snapshot(
        snapshot(),
        &[account("mail", "mail@example.test")],
        address,
        no_address,
        |_| None,
        no_change,
    );

    assert_eq!(
        state.calendar_outages,
        [
            OutageLine {
                account: "cal-only".to_owned(),
                text: outage_line(
                    AccountCapability::Calendar,
                    UseProblem::NeedsOwnSignIn,
                    "calendar-only@example.test"
                ),
                review: None,
            },
            OutageLine {
                account: "mail".to_owned(),
                text: outage_line(
                    AccountCapability::Calendar,
                    UseProblem::Unreachable,
                    "mail@example.test"
                ),
                review: None,
            },
        ]
    );
    assert_eq!(
        state.contacts_outages,
        [OutageLine {
            account: "mail".to_owned(),
            text: outage_line(
                AccountCapability::Contacts,
                UseProblem::SignInRefused,
                "mail@example.test"
            ),
            review: None,
        }]
    );
}

#[test]
fn a_use_paused_for_a_new_server_says_so_and_carries_the_change_to_review() {
    let mut paused = snapshot();
    paused.calendar_outages = vec![
        outage("cal-only", UseProblem::ServerChanged),
        outage("mail", UseProblem::ServerChanged),
    ];
    paused.contacts_outages = vec![outage("mail", UseProblem::Unreachable)];
    let state = ConnectivityState::from_snapshot(
        paused,
        &[account("mail", "mail@example.test")],
        address,
        no_address,
        |_| None,
        // A pause the core has since lifted has no change left to show.
        |id| (id == "cal-only").then(moved),
    );

    assert_eq!(
        state.calendar_outages,
        [
            OutageLine {
                account: "cal-only".to_owned(),
                text: outage_line(
                    AccountCapability::Calendar,
                    UseProblem::ServerChanged,
                    "calendar-only@example.test"
                ),
                review: Some(Review {
                    address: "calendar-only@example.test".to_owned(),
                    change: moved(),
                }),
            },
            OutageLine {
                account: "mail".to_owned(),
                text: outage_line(
                    AccountCapability::Calendar,
                    UseProblem::ServerChanged,
                    "mail@example.test"
                ),
                review: None,
            },
        ]
    );
    assert!(
        state.calendar_outages[0]
            .text
            .contains(crate::l10n::settings_account_use_server_changed_calendar())
    );
    assert_eq!(
        state.contacts_outages[0].review, None,
        "only a paused use asks about a server"
    );
}

#[test]
fn every_provider_routes_to_the_remedy_linux_currently_has() {
    let accounts = [
        account("m", "m@example.test"),
        account("j", "j@example.test"),
        account("i", "i@example.test"),
    ];
    let snapshot = ConnectivitySnapshot {
        offline: false,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: vec!["m".to_owned(), "j".to_owned(), "i".to_owned()],
        calendar_outages: Vec::new(),
        contacts_outages: Vec::new(),
    };
    let provider = |id: &str| {
        Some(match id {
            "m" => AccountProvider::Microsoft,
            "j" => AccountProvider::JmapOauth,
            _ => AccountProvider::ImapOauth,
        })
    };
    let state = ConnectivityState::from_snapshot(
        snapshot, &accounts, no_address, no_address, provider, no_change,
    );

    assert!(matches!(
        state.expired_signins[0].resolution(),
        ExpiredResolution::SignInAgain(account) if account == "m"
    ));
    assert!(matches!(
        state.expired_signins[1].resolution(),
        ExpiredResolution::JmapOauth(account) if account == "j"
    ));
    // An IMAP sign-in has no password for the account's page to ask for: the banner signs it in
    // again in place, as it does a JMAP one.
    assert!(matches!(
        state.expired_signins[2].resolution(),
        ExpiredResolution::ImapOauth(account) if account == "i"
    ));
}

#[test]
fn a_banner_asking_for_a_sign_in_names_the_account_apart_from_its_namesake() {
    let distinguished = |id: &str| {
        matches!(id, "expired" | "mail" | "calendar")
            .then(|| crate::l10n::settings_account_distinguished(id, "host.example.test"))
    };
    let state = ConnectivityState::from_snapshot(
        snapshot(),
        &[
            account("expired", "expired@example.test"),
            account("calendar", "calendar@example.test"),
            account("mail", "mail@example.test"),
        ],
        address,
        distinguished,
        |_| None,
        no_change,
    );

    assert_eq!(
        state.expired_signins[0].email,
        distinguished("expired").unwrap()
    );
    assert_eq!(
        state.expired_signins[1].email, "removed",
        "an account missing from the snapshot keeps the name it had"
    );
    assert_eq!(state.mail_reauth[0].email, distinguished("mail").unwrap());
    assert_eq!(
        state.calendar_reauth[0].email,
        distinguished("calendar").unwrap()
    );
}

#[test]
fn gio_reachability_crosses_the_relm_channel_with_its_value() {
    let (sender, receiver) = relm4::channel();

    report_reachability(&sender, false);

    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::NetworkReachabilityChanged(false))
    ));
}

pub(crate) fn the_banners_render_the_snapshot_and_keep_the_remedy_actionable() {
    let state = ConnectivityState::from_snapshot(
        snapshot(),
        &[
            account("expired", "expired@example.test"),
            account("mail", "mail@example.test"),
        ],
        address,
        no_address,
        |_| Some(AccountProvider::Google),
        no_change,
    );
    let (sender, receiver) = relm4::channel();
    let banners = ConnectivityBanners::new(&sender);

    banners.render(&state, PrimaryView::Mail);

    assert!(banners.offline.is_revealed());
    assert_eq!(
        banners.offline.title(),
        crate::l10n::connectivity_offline_banner()
    );
    assert!(banners.expired.is_revealed());
    assert!(banners.expired.title().contains("expired@example.test"));
    assert_eq!(
        banners.expired.button_label().as_deref(),
        Some(crate::l10n::signin_expired_action())
    );
    assert!(banners.mail_reauth.is_revealed());
    assert!(banners.mail_reauth.title().contains("mail"));
    assert!(!banners.calendar_reauth.is_revealed());

    banners
        .mail_reauth
        .emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ResolveMailReauth)
    ));

    banners.render(&state, PrimaryView::Calendar);
    assert!(!banners.mail_reauth.is_revealed());
    assert!(banners.calendar_reauth.is_revealed());
    assert!(banners.calendar_reauth.title().contains("calendar"));
    banners
        .calendar_reauth
        .emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ResolveCalendarReauth)
    ));

    banners.expired.emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ResolveExpiredSignIn)
    ));

    the_outage_lines_follow_the_snapshot_and_open_the_accounts_page(&banners, &receiver, &state);
}

fn the_outage_lines_follow_the_snapshot_and_open_the_accounts_page(
    banners: &ConnectivityBanners,
    receiver: &relm4::Receiver<AppInput>,
    state: &ConnectivityState,
) {
    banners.render(state, PrimaryView::Mail);
    assert!(!banners.calendar_outages.root.is_visible());
    assert!(!banners.contacts_outages.root.is_visible());

    banners.render(state, PrimaryView::Calendar);
    assert!(banners.calendar_outages.root.is_visible());
    assert!(!banners.contacts_outages.root.is_visible());
    let calendar = texts(&banners.calendar_outages);
    assert_eq!(
        calendar,
        state
            .calendar_outages
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
    );
    assert!(calendar[0].contains("calendar-only@example.test"));
    let first = banners
        .calendar_outages
        .root
        .first_child()
        .and_downcast::<adw::Banner>()
        .expect("a line per account");
    first.emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::Open(account))) if account == "cal-only"
    ));

    banners.render(state, PrimaryView::Contacts);
    assert!(!banners.calendar_outages.root.is_visible());
    assert!(banners.contacts_outages.root.is_visible());
    assert_eq!(
        texts(&banners.contacts_outages),
        [state.contacts_outages[0].text.clone()]
    );
    banners
        .contacts_outages
        .root
        .first_child()
        .and_downcast::<adw::Banner>()
        .expect("a line per account")
        .emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::Accounts(AccountsInput::Open(account))) if account == "mail"
    ));

    // The lines go when the snapshot stops naming them.
    let mut cleared = snapshot();
    cleared.calendar_outages.clear();
    cleared.contacts_outages.clear();
    let cleared =
        ConnectivityState::from_snapshot(cleared, &[], no_address, no_address, |_| None, no_change);
    banners.render(&cleared, PrimaryView::Calendar);
    assert!(!banners.calendar_outages.root.is_visible());
    assert_eq!(texts(&banners.calendar_outages), [] as [String; 0]);
    banners.render(&cleared, PrimaryView::Contacts);
    assert!(!banners.contacts_outages.root.is_visible());
    assert_eq!(texts(&banners.contacts_outages), [] as [String; 0]);

    // A paused use's line offers the review of its new server rather than the account's page.
    let mut paused = snapshot();
    paused.calendar_outages = vec![outage("mail", UseProblem::ServerChanged)];
    let paused = ConnectivityState::from_snapshot(
        paused,
        &[account("mail", "mail@example.test")],
        no_address,
        no_address,
        |_| None,
        |_| Some(moved()),
    );
    banners.render(&paused, PrimaryView::Calendar);
    let review = banners
        .calendar_outages
        .root
        .first_child()
        .and_downcast::<adw::Banner>()
        .expect("a line per account");
    assert_eq!(
        review.button_label().as_deref(),
        Some(crate::l10n::server_changed_review())
    );
}
