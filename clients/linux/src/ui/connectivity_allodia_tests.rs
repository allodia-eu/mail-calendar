//! The banner for an Allodia sign-in the account service refused (`entitlement.md`, "A refused
//! sign-in is also a banner").

use adw::prelude::*;
use mailcal_bindings::ConnectivitySnapshot;

use super::{ConnectivityBanners, ConnectivityState};
use crate::{
    l10n,
    ui::{
        AppInput, PrimaryView,
        allodia::{AllodiaOutcome, banner_notice},
    },
};

/// A snapshot with nothing to say, for a test to fill in the one field it is about.
pub(super) fn quiet() -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        offline: false,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: Vec::new(),
        calendar_outages: Vec::new(),
        contacts_outages: Vec::new(),
        allodia_sign_in_ended: None,
    }
}

fn state(ended: Option<&str>) -> ConnectivityState {
    ConnectivityState::from_snapshot(
        ConnectivitySnapshot {
            allodia_sign_in_ended: ended.map(str::to_owned),
            ..quiet()
        },
        &[],
        |_| None,
        |_| None,
        |_| None,
        |_| None,
    )
}

/// The address is the core's, and reaches the banner as it was sent: it is not a mail account, so
/// nothing in the account list names it.
#[test]
fn a_refused_allodia_sign_in_reaches_the_state_with_its_address() {
    assert_eq!(
        state(Some("someone@allodia.test"))
            .allodia_sign_in_ended
            .as_deref(),
        Some("someone@allodia.test")
    );
    assert_eq!(state(None).allodia_sign_in_ended, None);
}

/// A sign-in the banner started that did not complete says so, as the mail banner does; a dismissed
/// browser is the person's own choice and adds nothing, and one that worked lets the banner go.
#[test]
fn only_a_sign_in_that_did_not_complete_adds_a_sentence() {
    assert_eq!(
        banner_notice(&AllodiaOutcome::Failed("the exchange failed".to_owned())),
        Some(l10n::signin_expired_failed())
    );
    assert_eq!(banner_notice(&AllodiaOutcome::Cancelled), None);
    assert_eq!(banner_notice(&AllodiaOutcome::SignedIn), None);
}

/// Drawn on every view while the field is set, naming the address, with the one action that runs
/// the ordinary sign-in; gone once the field clears.
pub(crate) fn the_allodia_banner_names_the_address_and_signs_in() {
    let (sender, receiver) = relm4::channel();
    let banners = ConnectivityBanners::new(&sender);

    let ended = state(Some("someone@allodia.test"));
    for primary in [
        PrimaryView::Mail,
        PrimaryView::Calendar,
        PrimaryView::Contacts,
    ] {
        banners.render(&ended, primary);
        assert!(banners.allodia_ended.is_revealed(), "{primary:?}");
    }
    assert_eq!(
        banners.allodia_ended.title().as_str(),
        l10n::allodia_signin_ended_prompt("someone@allodia.test")
    );
    assert_eq!(
        banners.allodia_ended.button_label().as_deref(),
        Some(l10n::signin_expired_action())
    );
    assert!(
        !banners.expired.is_revealed(),
        "a refused Allodia sign-in is not a mail account's"
    );

    banners
        .allodia_ended
        .emit_by_name::<()>("button-clicked", &[]);
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ResolveAllodiaSignInEnded)
    ));

    banners.render(&state(None), PrimaryView::Mail);
    assert!(!banners.allodia_ended.is_revealed());
}
