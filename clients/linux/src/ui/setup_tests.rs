//! State regressions for the account-setup window.

use mailcal_bindings::{ConnectionSecurity, DetectedServerRow, ImapAuthOffer, SetupRecommendation};

use super::{Phase, SetupState};
use crate::ui::setup_model::{
    AccountKind, DetectedForm, ImapSignIn, JmapSignIn, ManualForm, SetupForm, manual_form,
    recommendation_form,
};

#[test]
fn a_slow_jmap_probe_cannot_change_a_newer_setup_form() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(jmap_form());

    assert!(!state.jmap_oauth_available("bob@other.test", "https://jmap.other.test", true));
    assert_eq!(detected_sign_in(&state), JmapSignIn::Checking);

    assert!(state.jmap_oauth_available("alice@example.test", "https://jmap.example.test", true));
    assert_eq!(detected_sign_in(&state), JmapSignIn::Offered);
}

#[test]
fn a_detected_card_renders_the_answer_either_way() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(jmap_form());
    let rendered_generation = state.generation;

    // The card holds neither the offer nor a secret field while it asks, so "no sign-in here"
    // is as much a change to what is on screen as an offer is.
    assert!(state.jmap_oauth_available("alice@example.test", "https://jmap.example.test", false));
    assert_ne!(state.generation, rendered_generation);
    assert_eq!(detected_sign_in(&state), JmapSignIn::Unavailable);
    assert!(detected_sign_in(&state).show_manual());
}

#[test]
fn only_the_first_answer_for_an_address_counts() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(jmap_form());

    // The deadline races the probe; a late real answer must not reopen a card the user is
    // already acting on.
    assert!(state.jmap_oauth_available("alice@example.test", "https://jmap.example.test", false));
    assert!(!state.jmap_oauth_available("alice@example.test", "https://jmap.example.test", true));
    assert_eq!(detected_sign_in(&state), JmapSignIn::Unavailable);
}

#[test]
fn switching_account_type_carries_the_address_and_asks_again() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(manual_form("alice@example.test".to_owned(), None));

    let probe = state.select_account_kind(ManualForm {
        kind: AccountKind::Jmap,
        email: "alice@example.test".to_owned(),
        // An answer carried over from another type is not an answer about this one.
        sign_in: JmapSignIn::Unavailable,
        ..ManualForm::default()
    });

    let probe = probe.expect("a JMAP type owes a pre-flight");
    assert_eq!(probe.email, "alice@example.test");
    assert_eq!(manual(&state).kind, AccountKind::Jmap);
    assert_eq!(manual(&state).sign_in, JmapSignIn::Checking);
}

#[test]
fn a_type_with_no_pre_flight_asks_nothing() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(manual_form(String::new(), None));

    for kind in [
        AccountKind::Imap,
        AccountKind::Microsoft,
        AccountKind::Google,
    ] {
        assert!(
            state
                .select_account_kind(ManualForm {
                    kind,
                    email: "alice@example.test".to_owned(),
                    ..ManualForm::default()
                })
                .is_none(),
            "{kind:?} has no server metadata to ask about",
        );
    }
}

#[test]
fn the_manual_jmap_pre_flight_runs_once_per_address_and_never_rebuilds() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(manual_form(String::new(), None));
    state.select_account_kind(ManualForm {
        kind: AccountKind::Jmap,
        email: "alice@example.test".to_owned(),
        ..ManualForm::default()
    });
    let rendered_generation = state.generation;

    // Leaving the field again with nothing changed does not spend a second round trip.
    assert!(
        state
            .adopt_manual_jmap(typed("alice@example.test"))
            .is_none()
    );
    state.jmap_oauth_available("alice@example.test", "", false);
    assert!(
        state
            .adopt_manual_jmap(typed("alice@example.test"))
            .is_none()
    );
    assert_eq!(
        state.generation, rendered_generation,
        "a manual pre-flight must not rebuild the pane over a secret being typed",
    );

    // A different address is a different question.
    let probe = state.adopt_manual_jmap(typed("bob@other.test"));
    assert_eq!(probe.expect("re-probe").email, "bob@other.test");
    assert_eq!(manual(&state).sign_in, JmapSignIn::Checking);
}

#[test]
fn a_probe_answer_reaches_the_manual_pane_too() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(manual_form(String::new(), None));
    state.select_account_kind(ManualForm {
        kind: AccountKind::Jmap,
        email: "alice@example.test".to_owned(),
        ..ManualForm::default()
    });

    assert!(state.jmap_oauth_available("alice@example.test", "", true));
    assert_eq!(manual(&state).sign_in, JmapSignIn::Offered);
    // And a failed sign-in hands the secret field back rather than dead-ending.
    state.jmap_sign_in_failed();
    assert_eq!(manual(&state).sign_in, JmapSignIn::Failed);
    assert!(manual(&state).sign_in.show_manual());
}

#[test]
fn a_detected_card_can_be_opened_as_its_own_account_type() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Microsoft {
            email: "person@outlook.com".to_owned(),
        },
        String::new(),
    ));

    assert!(
        state.edit_detected_manually().is_none(),
        "Microsoft owes no JMAP pre-flight",
    );
    assert_eq!(manual(&state).kind, AccountKind::Microsoft);
    assert_eq!(manual(&state).email, "person@outlook.com");
    // The email step is not where this lands; the form is what is on screen.
    assert_eq!(state.phase, Phase::Form);
}

#[test]
fn a_sign_in_phase_returns_to_the_form_it_started_from() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(recommendation_form(
        SetupRecommendation::Microsoft {
            email: "person@outlook.com".to_owned(),
        },
        String::new(),
    ));

    state.microsoft_signing_in();
    assert_eq!(state.phase, Phase::MicrosoftSigningIn);
    state.retry_form();
    assert_eq!(state.phase, Phase::Form);
    // A declined consent is shown on the card rather than swallowed.
    state.failed("access_denied".to_owned());
    assert_eq!(state.phase, Phase::Form);
    assert_eq!(state.error.as_deref(), Some("access_denied"));
}

fn typed(email: &str) -> ManualForm {
    ManualForm {
        kind: AccountKind::Jmap,
        email: email.to_owned(),
        ..ManualForm::default()
    }
}

fn manual(state: &SetupState) -> &ManualForm {
    let Some(SetupForm::Manual(form)) = state.form.as_ref() else {
        panic!("expected the manual form");
    };
    form
}

fn detected_sign_in(state: &SetupState) -> JmapSignIn {
    let Some(SetupForm::Detected(DetectedForm::Jmap(form))) = state.form.as_ref() else {
        panic!("expected the detected JMAP form");
    };
    form.sign_in
}

fn jmap_form() -> SetupForm {
    recommendation_form(
        SetupRecommendation::Jmap {
            email: "alice@example.test".to_owned(),
            server_url: "https://jmap.example.test".to_owned(),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    )
}

#[test]
fn only_the_first_imap_answer_for_a_server_counts() {
    // The deadline races the probe, exactly as it does for JMAP. A late real answer must not
    // reopen a card the user is already acting on: they may have started typing a password
    // into it, and rebuilding would take it away.
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(imap_form());

    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test:993",
        ImapAuthOffer::Password
    ));
    assert!(!state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test:993",
        ImapAuthOffer::SignIn {
            issuer: "https://login.example.test".to_owned(),
            provider_label: None,
            password_also_works: true,
        }
    ));
    assert_eq!(detected_imap_sign_in(&state), ImapSignIn::Password);
}

#[test]
fn a_slow_imap_probe_cannot_change_a_newer_setup_form() {
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(imap_form());

    assert!(!state.imap_auth_answered(
        "bob@other.test",
        "imap.other.test:993",
        ImapAuthOffer::Password
    ));
    assert_eq!(detected_imap_sign_in(&state), ImapSignIn::Checking);
}

#[test]
fn every_imap_answer_redraws_the_detected_card() {
    // The card shows neither a button nor a password field while it asks, so "a password, as
    // before" changes what is on screen every bit as much as an offer does.
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(imap_form());
    let rendered = state.generation;

    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test:993",
        ImapAuthOffer::Password
    ));
    assert_ne!(state.generation, rendered);
    assert!(detected_imap_sign_in(&state).show_password());
}

#[test]
fn the_manual_pane_only_redraws_when_the_answer_adds_something() {
    // Its password field is already on screen and stays whatever the answer is, so a rebuild
    // would erase a password being typed to say nothing. An offer, or the line explaining a
    // closed sign-in, is new and worth the rebuild.
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(SetupForm::Manual(typed_imap("alice@example.test")));
    let rendered = state.generation;

    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test",
        ImapAuthOffer::Password
    ));
    assert_eq!(
        state.generation, rendered,
        "a password answer changes nothing on the manual pane"
    );

    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(SetupForm::Manual(typed_imap("alice@example.test")));
    let rendered = state.generation;
    assert!(state.imap_auth_answered(
        "alice@example.test",
        "imap.example.test",
        ImapAuthOffer::RegistrationNeeded {
            password_also_works: true
        }
    ));
    assert_ne!(
        state.generation, rendered,
        "the line explaining a closed sign-in is new"
    );
}

#[test]
fn the_manual_imap_pre_flight_runs_once_per_server() {
    // Leaving the field a second time must not spend another dial at the provider.
    let mut state = SetupState::closed();
    state.open(false);
    state.show_form(SetupForm::Manual(typed_imap("alice@example.test")));

    assert!(
        state
            .adopt_manual_imap(typed_imap("alice@example.test"))
            .is_none(),
        "nothing typed has changed"
    );
    let probe = state
        .adopt_manual_imap(ManualForm {
            email: "bob@example.test".to_owned(),
            ..typed_imap("alice@example.test")
        })
        .expect("a new address is worth asking about");
    assert_eq!(probe.email, "bob@example.test");
    assert_eq!(manual(&state).imap_sign_in, ImapSignIn::Checking);
}

fn typed_imap(email: &str) -> ManualForm {
    ManualForm {
        kind: AccountKind::Imap,
        email: email.to_owned(),
        imap_host: "imap.example.test".to_owned(),
        ..ManualForm::default()
    }
}

fn detected_imap_sign_in(state: &SetupState) -> ImapSignIn {
    let Some(SetupForm::Detected(DetectedForm::Imap(form))) = state.form.as_ref() else {
        panic!("expected the detected IMAP form");
    };
    form.sign_in.clone()
}

fn imap_form() -> SetupForm {
    recommendation_form(
        SetupRecommendation::Imap {
            email: "alice@example.test".to_owned(),
            imap_host: "imap.example.test:993".to_owned(),
            smtp_host: None,
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
            incoming: DetectedServerRow {
                protocol: "IMAP".to_owned(),
                hostname: "imap.example.test".to_owned(),
                port: 993,
                security: "SSL/TLS".to_owned(),
                username: "alice@example.test".to_owned(),
            },
            outgoing: None,
            caldav_url: None,
            oauth_issuer: None,
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    )
}
