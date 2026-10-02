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
        setup::SetupWindow,
        setup_model::{AccountKind, manual_form, recommendation_form},
        setup_state::SetupState,
        setup_widget_tests::{
            check_button, descendant_button, descendant_has_button, descendants, drop_down, entries,
        },
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
    sender.emit(AppInput::CancelComposer(
        crate::ui::reader::ComposerHost::Pane,
    ));
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
    sender.emit(AppInput::CancelComposer(
        crate::ui::reader::ComposerHost::Pane,
    ));
    assert_eq!(
        receiver
            .recv_sync()
            .map(|input| format!("{input:?}"))
            .as_deref(),
        Some("CancelAccountSetup"),
        "closing a dismissible setup window must cancel the flow, not just hide it"
    );
}

/// A connect and its answer happen *in* the form, which stays on screen throughout. The
/// certificate panel is a question about the server the person just typed, so it is drawn into
/// their pane rather than replacing it: a rebuilt pane would ask for the whole server again,
/// and for the secret it never stored, in order to answer that question
/// (`docs/certificate-exceptions.md` rule 6, `docs/account-autodetect.md` rule 11).
pub(super) fn a_refusal_is_answered_in_the_form_it_came_from(window: &adw::ApplicationWindow) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let mut state = SetupState::closed();
    let mut setup = SetupWindow::default();
    state.open(false);
    state.show_form(manual_form("alice@example.test".to_owned(), None));
    setup.render(&state, window, &sender);
    let child = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("manual setup content");

    // Filled in as a person would: a server autodetection could not find, on a port nobody
    // standardised, which is what the manual form is for.
    let typed = entries(&child);
    typed[0].set_text("alice@example.test");
    typed[1].set_text("127.0.0.1");
    typed[2].set_text("1143");
    typed[3].set_text("a-password");
    let connect = descendant_button(&child, l10n::action_connect());
    assert!(connect.is_sensitive(), "a filled-in form can connect");

    // Pressed for real, because submitting is where the window used to be taken away: the
    // answer is drawn into this window, so there has to be a window left to draw it into.
    connect.emit_clicked();
    let dialog = setup.current_window().expect("the setup window");
    assert!(
        dialog.is_visible(),
        "submitting may not take the window away"
    );

    // The connect runs: the button gives way to the readout, and the form stays put.
    state.connecting();
    setup.render(&state, window, &sender);
    assert!(dialog.is_visible(), "nor may the connect that follows it");
    assert!(
        !connect.is_visible(),
        "a running connect stands where Connect was"
    );
    assert_eq!(
        setup.current_window().and_then(|window| window.child()),
        Some(child.clone()),
        "a connect may not replace the pane the person is typing in"
    );

    // And it is refused for a certificate.
    state.connect_failed(crate::ui::setup_model::ConnectFailure {
        message: Some("certificate not verified".to_owned()),
        certificate: Some(mailcal_bindings::RejectedCertificate {
            server_name: "127.0.0.1".to_owned(),
            sha256: "AB:CD:EF".to_owned(),
            subject_common_name: Some("127.0.0.1".to_owned()),
            subject_organisation: Some("Proton AG".to_owned()),
            issuer_common_name: Some("127.0.0.1".to_owned()),
            issuer_organisation: Some("Proton AG".to_owned()),
            not_before: None,
            not_after: None,
        }),
    });
    setup.render(&state, window, &sender);
    let refused = setup
        .current_window()
        .and_then(|window| window.child())
        .expect("refused manual content");
    assert_eq!(
        refused, child,
        "the refusal is drawn into the same pane, not into a new one"
    );

    // Every field stands, the secret included: it was never stored anywhere to be restored
    // from, and it does not have to be.
    let shown = entries(&refused);
    assert_eq!(shown[0].text(), "alice@example.test", "the address stands");
    assert_eq!(shown[1].text(), "127.0.0.1", "the server stands");
    assert_eq!(shown[2].text(), "1143", "the port stands");
    assert_eq!(shown[3].text(), "a-password", "the secret stands");

    // And the port is still *theirs*: the picker may not move it now any more than it could
    // before the connect.
    let security = descendants::<gtk::DropDown>(&refused)
        .into_iter()
        .nth(1)
        .expect("the mail server's security picker");
    security.set_selected(1 - security.selected());
    assert_eq!(
        entries(&refused)[2].text(),
        "1143",
        "a port the person typed stays theirs across a failed connect"
    );

    // Connect is back, and held until the certificate on screen has been accepted.
    assert!(connect.is_visible(), "the readout gives Connect back");
    assert!(
        !connect.is_sensitive(),
        "a certificate nobody has accepted cannot connect"
    );
    check_button(&refused, l10n::setup_certificate_confirm())
        .expect("the certificate must be offered for acceptance")
        .set_active(true);
    assert!(
        connect.is_sensitive(),
        "accepting it opens the retry, with everything still filled in"
    );
}
