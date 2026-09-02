//! What the detected IMAP card draws once its server has answered.
//!
//! Split from [`super::setup_widget_tests`], which had reached the size limit, along the seam
//! the cases already had: those assert what a *route* renders, these assert what one route
//! renders differently depending on what its server said.
//!
//! Driven through the state rather than by constructing a form, because the answer is what
//! decides which controls exist: a test that set the state directly would assert the card it
//! had just described rather than the one the flow produces.

use adw::prelude::*;
use mailcal_bindings::{ConnectionSecurity, ImapAuthOffer, SetupRecommendation};

use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::rendered_labels,
        setup::{SetupState, SetupWindow},
        setup_model::recommendation_form,
        setup_widget_tests::{descendants, server_row, visible_entries},
    },
};

/// Drives the pre-flight's answer in, as the flow does before the card is drawn.
pub(super) fn answer_password(state: &mut SetupState, email: &str, imap_host: &str) {
    assert!(
        state.imap_auth_answered(email, imap_host, ImapAuthOffer::Password),
        "the answer must reach the card it was asked for"
    );
}

/// While the server is being asked, the card shows what it is waiting for and nothing to act
/// on. A password field that appears and is then taken away reads as the app changing its
/// mind, and the answer decides whether it belongs there at all.
pub(super) fn a_detected_imap_card_shows_no_credential_until_the_server_answers(
    window: &adw::ApplicationWindow,
) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        imap_recommendation(true),
        String::new(),
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content");

    assert!(
        visible_entries(&child).is_empty(),
        "no credential field belongs on screen while the server is being asked"
    );
    let shown = rendered_labels(&child);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_imap_signin_checking()),
        "the card says what it is waiting for: {shown:?}"
    );
}

/// A provider that offers sign-in gets the button as the primary action, with the password
/// behind a secondary one: the order the provider's own answer puts them in.
pub(super) fn a_provider_offering_sign_in_leads_with_it(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        imap_recommendation(true),
        String::new(),
    ));
    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test:993",
        ImapAuthOffer::SignIn {
            issuer: "https://login.example.test".to_owned(),
            provider_label: None,
            password_also_works: true,
        },
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content");

    let labels: Vec<String> = descendants::<gtk::Button>(&child)
        .into_iter()
        .filter_map(|button| button.label().map(|label| label.to_string()))
        .collect();
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::setup_imap_signin_button()),
        "the sign-in button must be on screen: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label == l10n::setup_imap_signin_password_instead()),
        "and the password route stays reachable beside it: {labels:?}"
    );
    // Exactly one field on screen: the password behind that secondary action.
    assert_eq!(visible_entries(&child).len(), 1);
}

/// A provider whose sign-in exists but is closed to this application says so. Showing the same
/// bare password form as a provider with no OAuth at all leaves the user with no idea why the
/// button somebody else has is missing.
pub(super) fn a_provider_that_only_admits_registered_apps_says_so(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(recommendation_form(
        imap_recommendation(true),
        String::new(),
    ));
    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test:993",
        ImapAuthOffer::RegistrationNeeded {
            password_also_works: true
        },
    ));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("detected IMAP content");

    let shown = rendered_labels(&child);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::setup_imap_signin_registration_needed()),
        "the card explains why there is no sign-in button: {shown:?}"
    );
    let labels: Vec<String> = descendants::<gtk::Button>(&child)
        .into_iter()
        .filter_map(|button| button.label().map(|label| label.to_string()))
        .collect();
    assert!(
        !labels
            .iter()
            .any(|label| label == l10n::setup_imap_signin_button()),
        "no sign-in button, because there is no sign-in we can start: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == l10n::action_connect()),
        "the password route is the primary action here: {labels:?}"
    );
}

/// The detected IMAP recommendation the sign-in cases share.
pub(super) fn imap_recommendation(is_trusted: bool) -> SetupRecommendation {
    SetupRecommendation::Imap {
        oauth_issuer: None,
        email: "alice@example.test".to_owned(),
        imap_host: "imap.example.test:993".to_owned(),
        smtp_host: None,
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        incoming: server_row("IMAP", "imap.example.test", 993),
        outgoing: None,
        caldav_url: None,
        is_trusted,
        source: "fixture".to_owned(),
    }
}
