//! An add refused because the mailbox is an account already: what it keeps, and the offer it
//! draws. The widget function is called from the setup window's widget test.

use adw::prelude::*;
use mailcal_bindings::{MailcalError, SetupRecommendation};

use super::{ExistingInput, refusal, takes_update};
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::rendered_labels,
        setup::SetupWindow,
        setup_model::{AccountSubmission, ImapSubmission, JmapSubmission, recommendation_form},
        setup_state::SetupState,
        setup_widget_tests::{descendant_button, descendant_has_button},
    },
};

const EXISTING: &str = "alice@example.test@jmap.example.test";
const CONFIG: &str = "[jmap]\npassword = \"hunter2\"\n";

fn already_set_up() -> MailcalError {
    MailcalError::AlreadySetUp {
        account_id: EXISTING.to_owned(),
    }
}

#[test]
fn the_refusal_keeps_the_account_it_named_and_the_config_it_was_given() {
    let failure = refusal(already_set_up(), Some(CONFIG.to_owned()), false);
    assert_eq!(
        failure.message.as_deref(),
        Some(l10n::account_already_set_up())
    );
    let existing = failure.existing.expect("the account already set up");
    assert_eq!(existing.account_id, EXISTING);
    assert_eq!(existing.config_toml, CONFIG);
    assert!(
        !format!("{existing:?}").contains("hunter2"),
        "the config carries the credential and is never printed"
    );

    // A route with no credential to hand over says the same and offers nothing.
    let calendar = refusal(already_set_up(), None, true);
    assert_eq!(
        calendar.message.as_deref(),
        Some(l10n::account_already_set_up())
    );
    assert!(calendar.existing.is_none());

    // Any other refusal is said as before.
    let other = refusal(
        MailcalError::Connect("no route".to_owned()),
        Some(CONFIG.to_owned()),
        false,
    );
    assert!(other.existing.is_none());
    assert_eq!(other.message.as_deref(), Some("connect: no route"));
}

#[test]
fn only_a_mailbox_route_offers_its_credential_to_the_account_already_there() {
    let imap = |imap_host: &str| {
        AccountSubmission::Imap(ImapSubmission {
            email: "alice@example.test".to_owned(),
            imap_host: imap_host.to_owned(),
            smtp_host: String::new(),
            caldav_url: "https://dav.example.test".to_owned(),
            carddav_url: String::new(),
            uses: None,
            imap_security: mailcal_bindings::ConnectionSecurity::ImplicitTls,
            smtp_security: mailcal_bindings::ConnectionSecurity::ImplicitTls,
            password: "hunter2".to_owned(),
            accepted_certificate: None,
        })
    };
    assert!(takes_update(&imap("imap.example.test:993")));
    assert!(!takes_update(&imap("")), "a calendar alone has no mailbox");
    assert!(takes_update(&AccountSubmission::Jmap(JmapSubmission {
        email: "alice@example.test".to_owned(),
        server_url: String::new(),
        password: "hunter2".to_owned(),
    })));
}

fn jmap_card(state: &mut SetupState) {
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Jmap {
            email: "alice@example.test".to_owned(),
            server_url: "https://jmap.example.test".to_owned(),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    ));
}

/// The refusal offers to update the account already set up, in the pane that was refused,
/// whether the secret's Connect or a sign-in asked; and the offer asks once.
pub(crate) fn the_refusal_offers_the_existing_account_the_new_credential(
    window: &adw::ApplicationWindow,
) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    for sign_in in [false, true] {
        let mut state = SetupState::closed();
        let mut setup = SetupWindow::default();
        jmap_card(&mut state);
        assert!(state.jmap_oauth_available(
            "alice@example.test",
            "https://jmap.example.test",
            sign_in
        ));
        setup.render(&state, window, &sender);
        if sign_in {
            state.jmap_signing_in();
        } else {
            state.connecting();
        }
        setup.render(&state, window, &sender);
        state.connect_failed(refusal(already_set_up(), Some(CONFIG.to_owned()), false));
        setup.render(&state, window, &sender);
        let child = setup
            .current_window()
            .and_then(|window| window.child())
            .expect("refused JMAP content");
        let shown = rendered_labels(&child);
        assert!(
            shown
                .iter()
                .any(|text| text == l10n::account_already_set_up()),
            "the refusal is said: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|text| text == l10n::account_already_set_up_update_note()),
            "the offer says what it keeps: {shown:?}"
        );
        let update = descendant_button(&child, l10n::account_already_set_up_update());
        update.emit_clicked();
        assert!(matches!(
            receiver.recv_sync(),
            Some(AppInput::AlreadySetUp(ExistingInput::Update))
        ));
        assert!(!update.is_sensitive(), "the offer asks once");

        // Trying again takes the offer away until the next refusal makes it.
        state.connecting();
        setup.render(&state, window, &sender);
        state.connect_failed(refusal(
            MailcalError::Connect("no route".to_owned()),
            None,
            false,
        ));
        setup.render(&state, window, &sender);
        let child = setup
            .current_window()
            .and_then(|window| window.child())
            .expect("refused JMAP content");
        assert!(!descendant_has_button(
            &child,
            l10n::account_already_set_up_update()
        ));
    }
}
