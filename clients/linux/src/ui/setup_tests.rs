//! State regressions for the account-setup window.

use mailcal_bindings::SetupRecommendation;

use super::{Phase, SetupState};
use crate::ui::setup_model::{
    AccountKind, DetectedForm, JmapSignIn, ManualForm, SetupForm, manual_form, recommendation_form,
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
