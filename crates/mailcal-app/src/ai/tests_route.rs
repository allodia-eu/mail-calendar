//! Whether the feature is offered, and where a request goes (`docs/ai.md` rules 2, 4 and 5).

use std::sync::{Arc, Mutex};

use mailcal_ai::{
    Class, Destination, Mode,
    wire::{ChatMessage, ChatRequest, Purpose},
};

use super::{
    AiEndpointChange, AiEndpointEdit, AiEntitlement, AiFailure, AiRelay, AiRoute, AiWork,
    AiWorkKey, BuildKind, RelayStatus,
    tests_support::{FakeRelay, Fixture},
};
use crate::Surface;

fn save(fixture: &Fixture, address: &str, key: &str) {
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(AiEndpointEdit {
            address: address.to_owned(),
            model: "local-model".to_owned(),
            declared: Some(Class::EuNative),
            key: key.to_owned(),
        }));
}

fn request() -> ChatRequest {
    ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![ChatMessage::user("Hello".to_owned())],
        tools: Vec::new(),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    }
}

#[test]
fn a_production_build_offers_the_feature_only_while_the_entitlement_carries_it() {
    let fixture = Fixture::new(BuildKind::Production);
    assert!(!fixture.app.ai_offer());
    assert_eq!(fixture.app.ai_route(), Err(AiFailure::NotEntitled));

    fixture.app.set_ai_entitlement(AiEntitlement::Granted);
    assert!(fixture.app.ai_offer());
    for surface in [
        Surface::Settings,
        Surface::WritingStyle,
        Surface::ReplyDraft,
        Surface::AiEndpoint,
    ] {
        assert!(fixture.signalled(surface), "{surface:?}");
    }

    fixture.forget_surfaces();
    fixture.app.set_ai_entitlement(AiEntitlement::Granted);
    assert!(
        !fixture.signalled(Surface::WritingStyle),
        "an answer that changes nothing redraws nothing"
    );

    fixture.app.set_ai_entitlement(AiEntitlement::NotGranted);
    assert!(
        !fixture.app.ai_offer(),
        "an answer without it takes effect at once"
    );
    assert!(fixture.signalled(Surface::ReplyDraft));
}

#[test]
fn a_development_build_offers_it_with_no_entitlement() {
    let fixture = Fixture::new(BuildKind::Development);
    assert!(fixture.app.ai_offer());
}

#[test]
fn with_nothing_set_up_there_is_nowhere_to_send() {
    let fixture = Fixture::new(BuildKind::Development);
    assert_eq!(fixture.app.ai_route(), Err(AiFailure::NowhereToSend));
    let relay = Arc::new(FakeRelay::new(RelayStatus::SignedOut));
    fixture
        .app
        .install_ai_relay(Arc::clone(&relay) as Arc<dyn AiRelay>);
    assert_eq!(fixture.app.ai_route(), Err(AiFailure::NowhereToSend));
}

#[test]
fn the_relay_is_the_route_when_no_endpoint_is_set_up_and_says_when_to_sign_in_again() {
    let fixture = Fixture::new(BuildKind::Development);
    let relay = Arc::new(FakeRelay::new(RelayStatus::Ready));
    fixture
        .app
        .install_ai_relay(Arc::clone(&relay) as Arc<dyn AiRelay>);
    assert_eq!(fixture.app.ai_route(), Ok(AiRoute::Relay));
    *relay.status.lock().unwrap() = RelayStatus::SignInNeeded;
    assert_eq!(fixture.app.ai_route(), Err(AiFailure::RelaySignIn));
}

#[test]
fn an_own_endpoint_wins_over_the_relay() {
    let fixture = Fixture::new(BuildKind::Development);
    let relay = Arc::new(FakeRelay::new(RelayStatus::Ready));
    fixture
        .app
        .install_ai_relay(Arc::clone(&relay) as Arc<dyn AiRelay>);
    save(&fixture, "https://models.example.eu/v1", "");
    assert_eq!(
        fixture.app.ai_route(),
        Ok(AiRoute::OwnEndpoint {
            host: "models.example.eu".to_owned(),
            declared: Some(Class::EuNative),
        })
    );
}

/// An endpoint stored when the mode was wider: the gate refuses it now, before anything is read.
#[test]
fn the_gate_refuses_an_endpoint_the_mode_in_force_does_not_admit() {
    let fixture = Fixture::stored(BuildKind::Development, |dir| {
        std::fs::write(
            dir.join("preferences.toml"),
            "[ai]\njurisdiction = \"eu-native\"\n\n[ai.endpoint]\n\
             base_url = \"https://models.example.com/v1\"\nmodel = \"m\"\ndeclared = \"non-eu\"\n",
        )
        .unwrap();
    });
    assert_eq!(fixture.app.ai_mode(), Mode::EuNative);
    assert_eq!(
        fixture.app.ai_route(),
        Err(AiFailure::Refused {
            mode: Mode::EuNative,
            class: Class::NonEu,
        })
    );
}

/// The backend a piece of work is handed sends to the own endpoint, with the key from the
/// keystore, under the destination the endpoint's declaration gives it.
#[test]
fn work_sends_to_the_own_endpoint_with_its_key() {
    let fixture = Fixture::new(BuildKind::Development);
    save(
        &fixture,
        " https://models.example.eu/v1/ ",
        " sk-test ",
    );
    let seen = Arc::new(Mutex::new(None));
    let destination = Arc::clone(&seen);
    fixture
        .app
        .run_ai_work_now(AiWork::new(AiWorkKey::Learning, move |app, run| {
            let backend = app.ai_backend(run).expect("a backend");
            *destination.lock().unwrap() = Some(backend.destination());
            let _ = backend.chat(&request());
        }));
    assert_eq!(
        *seen.lock().unwrap(),
        Some(Destination::OwnEndpoint {
            declared: Some(Class::EuNative)
        })
    );
    let requests = &fixture.host.seen.lock().unwrap().requests;
    assert_eq!(
        requests.as_slice(),
        [(
            "https://models.example.eu/v1/chat/completions".to_owned(),
            Some("sk-test".to_owned())
        )]
    );
}

/// The relay's refusal of the sign-in is "sign in again", decided from the destination.
#[test]
fn work_sends_to_the_relay_when_no_endpoint_is_set_up() {
    let fixture = Fixture::new(BuildKind::Development);
    let relay = Arc::new(FakeRelay::new(RelayStatus::Ready));
    fixture
        .app
        .install_ai_relay(Arc::clone(&relay) as Arc<dyn AiRelay>);
    let failure = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&failure);
    fixture
        .app
        .run_ai_work_now(AiWork::new(AiWorkKey::Learning, move |app, run| {
            let backend = app.ai_backend(run).expect("a backend");
            let error = backend
                .chat(&request())
                .expect_err("the fake relay refuses");
            *seen.lock().unwrap() = AiFailure::of_request(&error, backend.destination());
        }));
    assert_eq!(*relay.asked.lock().unwrap(), 1);
    assert_eq!(*failure.lock().unwrap(), Some(AiFailure::RelaySignIn));
}
