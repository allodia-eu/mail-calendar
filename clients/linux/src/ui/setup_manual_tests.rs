//! Widget-level regressions for the setup window's **manual** form and its window lifecycle:
//! the account-type picker, the line explaining a miss, and how a phase change and a user close
//! behave.
//!
//! The sibling of [`super::setup_widget_tests`], which covers the detected routes, and it shares
//! that file's tree helpers. Functions rather than `#[test]`s, for the reason given there.

use adw::prelude::*;
use mailcal_bindings::{MissReason, SetupRecommendation};

use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::{self, tests::rendered_labels},
        setup::{SetupState, SetupWindow},
        setup_model::{AccountKind, manual_form, recommendation_form},
        setup_widget_tests::{descendant_has_button, drop_down, entries},
        welcome::WelcomeWindow,
    },
};

pub(super) fn a_guarded_welcome_dismisses_on_consent(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut welcome = WelcomeWindow::default();
    welcome.render(true, window, None, sender.clone());
    let welcome_window = welcome.current_window().expect("welcome window");
    assert!(welcome_window.is_visible());
    mailbox::tests::every_row_belongs_to_a_list(welcome_window.upcast_ref::<gtk::Widget>());
    welcome.render(false, window, None, sender);
    assert!(
        !welcome_window.is_visible(),
        "consent completion must dismiss the guarded welcome window"
    );
}

pub(super) fn required_phases_swap_content_instead_of_stacking(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    state.open(true);
    let mut setup = SetupWindow::default();
    setup.render(&state, window, &sender);
    let email_window = setup.current_window().expect("setup email window");
    assert!(email_window.is_visible());
    email_window.close();
    assert!(
        email_window.is_visible(),
        "required setup must reject a user close"
    );

    let email_content = email_window.child().expect("setup email content");
    state.connecting();
    setup.render(&state, window, &sender);
    let connecting_window = setup.current_window().expect("connecting window");
    assert_eq!(
        connecting_window, email_window,
        "a phase change swaps the modal's content, so phases cannot stack on each other"
    );
    assert_ne!(
        connecting_window.child().expect("connecting content"),
        email_content,
        "the phase's content must actually be replaced"
    );
    assert!(
        connecting_window.default_widget().is_none(),
        "a phase with no primary action must clear the previous phase's default widget"
    );
    assert!(connecting_window.is_visible());

    state.complete();
    setup.render(&state, window, &sender);
    assert!(
        !connecting_window.is_visible(),
        "successful account setup must dismiss the guarded progress window"
    );
}

pub(super) fn the_manual_form_switches_account_type(window: &adw::ApplicationWindow) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(manual_form("alice@example.test".to_owned(), None));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("manual setup content");

    let picker = drop_down(&child).expect("the manual form must offer an account type");
    assert_eq!(picker.selected(), AccountKind::Imap.position());
    // JMAP rather than one of the browser sign-ins: those are offered only when the build
    // carries the provider's OAuth client registration, and a kind that is not offered has no
    // position of its own to select.
    picker.set_selected(AccountKind::Jmap.position());
    // A sentinel behind the expected message, so this read cannot block when the picker is
    // unwired; it fails instead.
    sender.emit(AppInput::CancelComposer);
    assert_eq!(
        receiver
            .recv_sync()
            .map(|input| format!("{input:?}"))
            .as_deref(),
        Some("SelectAccountKind"),
        "choosing an account type must reach the model"
    );

    // And the type it switched to renders its own surface, carrying the address across. Done on
    // Microsoft, whose surface is a button rather than a form, so it also proves the switch
    // replaced the fields; skipped when this build offers no Microsoft route to switch to.
    if !mailcal_bindings::oauth_routes().microsoft {
        return;
    }
    state.select_account_kind(crate::ui::setup_model::ManualForm {
        kind: AccountKind::Microsoft,
        email: "alice@example.test".to_owned(),
        ..crate::ui::setup_model::ManualForm::default()
    });
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("manual Microsoft content");
    assert!(descendant_has_button(
        &child,
        l10n::setup_microsoft_signin()
    ));
    assert!(
        entries(&child)
            .iter()
            .any(|field| field.text() == "alice@example.test"),
        "the address survives a change of account type"
    );
}

/// Detection that finds nothing hands over to the manual form **with a line saying why**; a
/// form that simply appears reads as the app having ignored the address.
pub(super) fn a_miss_explains_itself_on_the_manual_form(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);

    for (reason, expected) in [
        (
            MissReason::NothingFound,
            l10n::setup_detect_reason_nothing(),
        ),
        (
            MissReason::NetworkError,
            l10n::setup_detect_reason_network(),
        ),
        (
            MissReason::OauthOnlyProvider,
            l10n::setup_detect_reason_oauth_only(),
        ),
    ] {
        state.show_form(recommendation_form(
            SetupRecommendation::Manual { reason },
            "alice@example.test".to_owned(),
        ));
        setup.render(&state, window, &sender);
        let child = setup
            .current_window()
            .and_then(|window| window.child())
            .expect("manual content after a miss");
        let shown = rendered_labels(&child);
        assert!(
            shown.iter().any(|text| text == expected),
            "a miss must say why it sent the user here: {shown:?}"
        );
        // The address they already typed carries over rather than being asked for twice.
        assert!(
            entries(&child)
                .iter()
                .any(|field| field.text() == "alice@example.test"),
            "the typed address carries into the manual form"
        );
    }
}

pub(super) fn a_dismissible_window_cancels_the_flow(window: &adw::ApplicationWindow) {
    // Closing with the window controls must end the flow, not merely hide it. Without the
    // cancel the state stays `visible`, so a late bump: a slow JMAP pre-flight, a detection
    // result, a connect failure; re-presents the modal the user had just dismissed.
    let (sender, receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    setup.render(&state, window, &sender);
    let dismissible = setup.current_window().expect("dismissible setup window");
    dismissible.close();
    assert!(
        !dismissible.is_visible(),
        "a dismissible setup window must accept a user close"
    );
    sender.emit(AppInput::CancelComposer);
    assert_eq!(
        receiver
            .recv_sync()
            .map(|input| format!("{input:?}"))
            .as_deref(),
        Some("CancelAccountSetup"),
        "closing a dismissible setup window must cancel the flow, not just hide it"
    );
}
