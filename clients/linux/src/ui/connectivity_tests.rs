//! Regressions for the Linux host's connectivity projection and reachability bridge.

use adw::prelude::*;
use mailcal_bindings::{AccountProvider, AccountRow, ConnectivitySnapshot};

use super::{ConnectivityBanners, ConnectivityState, ExpiredResolution, report_reachability};
use crate::ui::{AppInput, PrimaryView};

fn account(id: &str, email: &str) -> AccountRow {
    AccountRow {
        id: id.to_owned(),
        email: email.to_owned(),
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
    }
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
        |id| (id == "expired").then_some(AccountProvider::Google),
    );

    assert!(state.offline);
    assert!(state.unreachable_accounts.contains("outage"));
    assert_eq!(state.calendar_reauth_emails, ["calendar@example.test"]);
    assert_eq!(state.mail_reauth_emails, ["mail@example.test"]);
    assert_eq!(state.expired_signins.len(), 2);
    assert_eq!(state.expired_signins[0].id, "expired");
    assert_eq!(state.expired_signins[0].email, "expired@example.test");
    assert!(matches!(
        state.expired_signins[0].resolution(),
        ExpiredResolution::Google(email) if email == "expired@example.test"
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
fn every_provider_routes_to_the_remedy_linux_currently_has() {
    let accounts = [
        account("m", "m@example.test"),
        account("j", "j@example.test"),
    ];
    let snapshot = ConnectivitySnapshot {
        offline: false,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: vec!["m".to_owned(), "j".to_owned()],
    };
    let state = ConnectivityState::from_snapshot(snapshot, &accounts, |id| {
        Some(if id == "m" {
            AccountProvider::Microsoft
        } else {
            AccountProvider::JmapOauth
        })
    });

    assert!(matches!(
        state.expired_signins[0].resolution(),
        ExpiredResolution::Microsoft(email) if email == "m@example.test"
    ));
    assert!(matches!(
        state.expired_signins[1].resolution(),
        ExpiredResolution::JmapOauth(account) if account == "j"
    ));
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
        &[account("expired", "expired@example.test")],
        |_| Some(AccountProvider::Google),
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
}
