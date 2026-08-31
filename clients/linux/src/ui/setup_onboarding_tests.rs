//! Widget-level regressions for the first-run Allodia card ([`super::setup_onboarding`]).
//!
//! Functions rather than `#[test]`s, called from the crate's single GTK test: see
//! [`super::mailbox::thread_tests`] for why there is exactly one.
//!
//! What is worth asserting here is the contract's order and its one silent failure. A build with
//! no registration must lose the card, the sign-in line **and** the divider together: dropping
//! only the first two renders perfectly and leaves a heading naming nothing.

use mailcal_bindings::{AllodiaAccountKind, AllodiaAccountOffer};

use super::{
    AppInput,
    mailbox::tests::{every_row_belongs_to_a_list, rendered_labels},
    setup::{SetupState, SetupWindow},
    setup_onboarding::{Onboarding, Progress},
    setup_widget_tests::{descendant_has_button, visible_entries},
};
use crate::l10n;

pub(super) fn the_first_account_screen_offers_an_allodia_account(window: &adw::ApplicationWindow) {
    the_card_sits_above_the_address_field(window);
    a_build_without_a_registration_loses_all_three(window);
    a_second_account_is_never_pitched_the_card_again(window);
    a_later_add_still_offers_the_accounts_not_set_up_yet(window);
    a_later_add_with_nothing_to_offer_is_the_direct_route_alone(window);
    signing_in_replaces_the_card_with_what_the_other_devices_hold(window);
    a_first_device_says_it_found_nothing_rather_than_going_quiet(window);
    a_screen_opened_after_the_consent_window_still_carries_the_card(window);
    a_browser_hop_that_does_not_come_back_can_be_escaped(window);
    a_pass_that_has_not_answered_is_not_an_empty_account(window);
}

/// "We have not looked" and "there is nothing" are different answers, and only the second may be
/// put on screen.
///
/// A pass that failed; the service down, the network gone; leaves no report. Reporting that as
/// "no mail accounts yet, add your first" tells somebody their account is empty on the strength of
/// a question that was never answered.
fn a_pass_that_has_not_answered_is_not_an_empty_account(window: &adw::ApplicationWindow) {
    let shown = rendered_labels(&first_account_screen(
        window,
        Onboarding {
            progress: Progress::SignedIn,
            offers: None,
            ..offered()
        },
    ));

    assert!(
        !shown
            .iter()
            .any(|text| text == l10n::setup_allodia_none_title()),
        "a pass with no answer must not be reported as an empty account: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_allodia_divider()),
        "the divider goes with the card in every state: {shown:?}"
    );
}

/// The screen nobody can skip must not be able to strand somebody.
///
/// A sign-in that goes wrong in the browser; or on the service behind it; leaves this card
/// spinning against a listener with a five-minute cap, on a window that rejects a close. So the
/// way back appears once the hop has outlasted [`SIGN_IN_ESCAPE_AFTER`], and not before: an
/// ordinary hop is in front of the person within a second, and a button drawn for that one is
/// noise on every successful sign-in there will ever be.
fn a_browser_hop_that_does_not_come_back_can_be_escaped(window: &adw::ApplicationWindow) {
    let hopping = |escapable| Onboarding {
        progress: Progress::SigningIn { escapable },
        ..offered()
    };

    let waiting = first_account_screen(window, hopping(false));
    assert!(
        rendered_labels(&waiting)
            .iter()
            .any(|text| text == l10n::settings_allodia_signing_in()),
        "the card says what it is waiting for"
    );
    assert!(
        !descendant_has_button(&waiting, l10n::action_cancel()),
        "an ordinary hop draws no way back it has not earned"
    );

    assert!(
        descendant_has_button(
            &first_account_screen(window, hopping(true)),
            l10n::action_cancel()
        ),
        "a hop past its threshold owes the person a way out"
    );
}

/// On a true first run the consent screen is answered first, and the first-account screen is
/// opened from that answer; several inputs later, and not by the code that knows whether this
/// build has a registration. So what the card is told at boot has to survive being opened after.
///
/// The failure this pins renders perfectly: the screen falls back to the direct route alone, which
/// is exactly what a build with no registration is supposed to look like.
fn a_screen_opened_after_the_consent_window_still_carries_the_card(
    window: &adw::ApplicationWindow,
) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();

    state.set_onboarding(offered());
    state.open(true);
    setup.render(&state, window, &sender);
    let shown = rendered_labels(&child_of(&setup));

    assert!(
        shown.iter().any(|text| text == l10n::setup_allodia_title()),
        "the card set before the window opened must still be on it: {shown:?}"
    );
}

/// The order is the rule: card, sign-in line, divider, then the address field.
fn the_card_sits_above_the_address_field(window: &adw::ApplicationWindow) {
    let screen = first_account_screen(window, offered());
    let shown = rendered_labels(&screen);

    let at = |needle: &str| {
        shown
            .iter()
            .position(|text| text == needle)
            .unwrap_or_else(|| panic!("{needle:?} is not on screen: {shown:?}"))
    };
    let card = at(l10n::setup_allodia_title());
    let sign_in = at(l10n::setup_allodia_have_one());
    let divider = at(l10n::setup_allodia_divider());
    let field = at(l10n::setup_detect_description());
    assert!(
        card < sign_in && sign_in < divider && divider < field,
        "the card, the sign-in line and the divider come above the address field: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_allodia_subtitle()),
        "the card says what the account does: {shown:?}"
    );

    // The action, not the "Recommended" marker, is what the card's control is called.
    assert!(descendant_has_button(&screen, l10n::setup_allodia_create()));
}

/// The rule that fails while rendering perfectly. `allodia_sign_in_available()` is false in a
/// build carrying no registration, and all three items go with it: a lone "or connect directly"
/// heading under nothing is the tell that the wrong thing was gated.
fn a_build_without_a_registration_loses_all_three(window: &adw::ApplicationWindow) {
    let screen = first_account_screen(
        window,
        Onboarding {
            offered: false,
            ..offered()
        },
    );
    let shown = rendered_labels(&screen);

    for absent in [
        l10n::setup_allodia_title(),
        l10n::setup_allodia_subtitle(),
        l10n::setup_allodia_have_one(),
        l10n::setup_allodia_divider(),
    ] {
        assert!(
            !shown.iter().any(|text| text == absent),
            "a build with no registration must not draw {absent:?}: {shown:?}"
        );
    }
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_detect_description()),
        "the direct route is the whole screen without a registration: {shown:?}"
    );
    assert_eq!(
        visible_entries(&screen).len(),
        1,
        "the address field is still the one thing to fill in"
    );
}

/// The **card** is first-account-only: somebody who has decided is not pitched again.
fn a_second_account_is_never_pitched_the_card_again(window: &adw::ApplicationWindow) {
    let shown = rendered_labels(&render(window, false, offered()));

    for absent in [
        l10n::setup_allodia_title(),
        l10n::setup_allodia_have_one(),
        l10n::setup_allodia_divider(),
    ] {
        assert!(
            !shown.iter().any(|text| text == absent),
            "somebody adding their second account is not asked again about {absent:?}: {shown:?}"
        );
    }
}

/// The **offers** are not first-account-only, and that is a different question from the card.
///
/// Somebody with three linked accounts sets one up and the window closes. Gating the offers with
/// the card left the other two reachable only from a Settings page: while the "Add account…"
/// button beside it asked them to type an address they could have picked from a list.
fn a_later_add_still_offers_the_accounts_not_set_up_yet(window: &adw::ApplicationWindow) {
    let signed_in = Onboarding {
        progress: Progress::SignedIn,
        offers: Some(vec![offer("carol@example.test")]),
        ..offered()
    };
    let shown = rendered_labels(&render(window, false, signed_in));

    assert!(
        shown.iter().any(|text| text == "carol@example.test"),
        "an account still to set up is offered on a later add too: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_allodia_divider()),
        "and the divider names what the field below it is: {shown:?}"
    );
    assert!(
        !shown.iter().any(|text| text == l10n::setup_allodia_title()),
        "without pitching the card again: {shown:?}"
    );
}

/// Nothing left to offer is the ordinary second add, and draws nothing at all.
///
/// Not even the empty-answer message: that is a first-run sentence about an account with no mail
/// accounts, and somebody adding their second has one. A divider over nothing is the shape the
/// contract's own rule forbids.
fn a_later_add_with_nothing_to_offer_is_the_direct_route_alone(window: &adw::ApplicationWindow) {
    for offers in [None, Some(Vec::new())] {
        let shown = rendered_labels(&render(
            window,
            false,
            Onboarding {
                progress: Progress::SignedIn,
                offers,
                ..offered()
            },
        ));
        for absent in [
            l10n::setup_allodia_divider(),
            l10n::setup_allodia_none_title(),
            l10n::settings_allodia_sync_heading(),
        ] {
            assert!(
                !shown.iter().any(|text| text == absent),
                "a later add with nothing to offer draws no {absent:?}: {shown:?}"
            );
        }
    }
}

/// Signing in is not a detour: what the other devices hold **replaces** the card, each account a
/// row with its address and a Set up button. The divider stays; it still names what follows.
fn signing_in_replaces_the_card_with_what_the_other_devices_hold(window: &adw::ApplicationWindow) {
    let screen = first_account_screen(
        window,
        Onboarding {
            progress: Progress::SignedIn,
            offers: Some(vec![offer("alice@example.test")]),
            ..offered()
        },
    );
    let shown = rendered_labels(&screen);

    assert!(
        shown.iter().any(|text| text == "alice@example.test"),
        "an offered account is named by its address: {shown:?}"
    );
    assert!(
        !shown.iter().any(|text| text == l10n::setup_allodia_title()),
        "the offers replace the card rather than joining it: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_allodia_divider()),
        "the divider still names what follows: {shown:?}"
    );
    assert!(descendant_has_button(
        &screen,
        l10n::settings_allodia_sync_set_up()
    ));
    assert!(
        !shown
            .iter()
            .any(|text| text == l10n::setup_allodia_none_title()),
        "an account that did come over is not 'no mail accounts yet': {shown:?}"
    );
}

/// A first device has nothing to bring over, and the screen has to say so.
///
/// Left to itself this state draws a divider over an address field, with the card gone; which
/// reads as the sign-in having failed rather than having found nothing. The heading over an empty
/// list would be worse still, so what replaces both is a sentence.
fn a_first_device_says_it_found_nothing_rather_than_going_quiet(window: &adw::ApplicationWindow) {
    let shown = rendered_labels(&first_account_screen(
        window,
        Onboarding {
            progress: Progress::SignedIn,
            offers: Some(Vec::new()),
            ..offered()
        },
    ));

    for expected in [
        l10n::setup_allodia_none_title(),
        l10n::setup_allodia_none_body(),
    ] {
        assert!(
            shown.iter().any(|text| text == expected),
            "a first device is told what was found and what to do: {shown:?}"
        );
    }
    assert!(
        !shown
            .iter()
            .any(|text| text == l10n::settings_allodia_sync_heading()),
        "a heading with nothing under it is worse than no heading: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_allodia_divider()),
        "the divider goes with the card in every state: {shown:?}"
    );
}

/// The state a registered build opens the first-account screen in, before anyone has signed in.
fn offered() -> Onboarding {
    Onboarding {
        offered: true,
        ..Onboarding::default()
    }
}

fn offer(email: &str) -> AllodiaAccountOffer {
    AllodiaAccountOffer {
        id: format!("record-{email}"),
        email: email.to_owned(),
        kind: AllodiaAccountKind::Imap,
        host: Some("imap.example.test".to_owned()),
        port: Some(993),
        security: None,
        smtp_host: None,
        smtp_port: None,
        smtp_security: None,
        caldav_base_url: None,
        jmap_base_url: None,
    }
}

/// The required (first-account) screen, with the focus rule checked on the way out.
///
/// The card is the only part of this window built from list rows, and a row appended to a plain
/// box renders and is then skipped by the keyboard; invisible to every assertion above.
fn first_account_screen(window: &adw::ApplicationWindow, state: Onboarding) -> gtk::Widget {
    let screen = render(window, true, state);
    every_row_belongs_to_a_list(&screen);
    screen
}

fn render(window: &adw::ApplicationWindow, required: bool, onboarding: Onboarding) -> gtk::Widget {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(required);
    state.set_onboarding(onboarding);
    setup.render(&state, window, &sender);
    child_of(&setup)
}

fn child_of(setup: &SetupWindow) -> gtk::Widget {
    setup
        .current_window()
        .and_then(|window| gtk::prelude::GtkWindowExt::child(&window))
        .expect("the account-setup screen")
}
