//! A kept key goes only to the server it was given for (`docs/ai.md` rule 7): a save that moves
//! the address to another host or port, with no new key typed, takes the kept key out, and the
//! form says so before the press through the same function the save acts on.

use mailcal_ai::Class;

use super::{
    AiEndpointChange, AiEndpointEdit, AiEndpointKeptKey, AiEndpointResult, BuildKind,
    tests_support::Fixture,
};

fn save(fixture: &Fixture, address: &str, key: &str) {
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(AiEndpointEdit {
            address: address.to_owned(),
            model: "m".to_owned(),
            declared: Some(Class::EuNative),
            key: key.to_owned(),
        }));
}

fn result(fixture: &Fixture) -> AiEndpointResult {
    fixture
        .app
        .ai_endpoint()
        .outcome
        .expect("an outcome")
        .result
}

fn kept_key(fixture: &Fixture, address: &str) -> Option<AiEndpointKeptKey> {
    fixture
        .app
        .ai_endpoint_gap(address, "m", Some(Class::EuNative))
        .kept_key
}

/// Saved with a key, at `https://api.example.eu/v1`.
fn with_a_kept_key() -> Fixture {
    let fixture = Fixture::new(BuildKind::Development);
    save(&fixture, "https://api.example.eu/v1", "sk-eu");
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    fixture
}

#[test]
fn nothing_is_said_about_a_key_while_none_is_kept() {
    let fixture = Fixture::new(BuildKind::Development);
    assert_eq!(kept_key(&fixture, "https://api.example.eu/v1"), None);
    save(&fixture, "https://api.example.eu/v1", "");
    assert_eq!(kept_key(&fixture, "https://api.example.com/v1"), None);
}

#[test]
fn the_form_says_whether_the_kept_key_goes_with_the_address_as_typed() {
    let fixture = with_a_kept_key();
    for same in [
        "https://api.example.eu/v2",
        "https://API.example.eu:443/v1/",
        "http://api.example.eu/v1",
    ] {
        assert_eq!(
            kept_key(&fixture, same),
            Some(AiEndpointKeptKey::Carried),
            "{same}"
        );
    }
    for other in [
        "https://api.example.com/v1",
        "https://api.example.eu:8443/v1",
        "not a url",
        "",
    ] {
        assert_eq!(
            kept_key(&fixture, other),
            Some(AiEndpointKeptKey::Dropped),
            "{other}"
        );
    }
    assert!(
        fixture
            .app
            .ai_endpoint_gap("https://api.example.com/v1", "m", Some(Class::EuNative))
            .is_empty(),
        "a key that will not be carried refuses nothing"
    );
}

#[test]
fn a_path_or_scheme_change_on_the_same_server_keeps_the_key() {
    let fixture = with_a_kept_key();
    save(&fixture, "https://api.example.eu/v2", "");
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    assert!(fixture.app.ai_endpoint().fields.key_kept);
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk-eu".to_owned())],
        "the keystore is not asked"
    );
}

/// ⚠️ Without this, pointing the endpoint at another provider sends it the first one's key.
#[test]
fn another_server_without_a_new_key_takes_the_kept_key_out() {
    let fixture = with_a_kept_key();
    assert_eq!(
        kept_key(&fixture, "https://api.example.com/v1"),
        Some(AiEndpointKeptKey::Dropped)
    );
    save(&fixture, "https://api.example.com/v1", "");
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    let view = fixture.app.ai_endpoint();
    assert!(view.stored);
    assert!(!view.fields.key_kept);
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk-eu".to_owned()), None]
    );
    assert_eq!(kept_key(&fixture, "https://api.example.com/v1"), None);
}

#[test]
fn another_server_with_a_new_key_keeps_the_new_key() {
    let fixture = with_a_kept_key();
    save(&fixture, "https://api.example.com/v1", "sk-com");
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    assert!(fixture.app.ai_endpoint().fields.key_kept);
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk-eu".to_owned()), Some("sk-com".to_owned())]
    );
}

#[test]
fn a_key_kept_with_no_endpoint_stored_goes_to_no_new_address() {
    let fixture = Fixture::new(BuildKind::Development);
    fixture
        .app
        .restore_ai_endpoint_key(Some("sk-old".to_owned()));
    assert_eq!(
        kept_key(&fixture, "https://api.example.eu/v1"),
        Some(AiEndpointKeptKey::Dropped)
    );
    save(&fixture, "https://api.example.eu/v1", "");
    assert!(!fixture.app.ai_endpoint().fields.key_kept);
    assert_eq!(fixture.host.seen.lock().unwrap().keys, [None]);
}

#[test]
fn a_keystore_that_refuses_to_take_the_key_out_changes_nothing() {
    let fixture = with_a_kept_key();
    *fixture.host.refuse_keys.lock().unwrap() = true;
    save(&fixture, "https://api.example.com/v1", "");
    assert_eq!(result(&fixture), AiEndpointResult::KeyNotStored);
    let view = fixture.app.ai_endpoint();
    assert!(view.fields.key_kept);
    assert_eq!(
        view.fields.address, "https://api.example.com/v1",
        "the form keeps the edit"
    );
    let stored = fixture.app.ai.settings().endpoint.clone().unwrap();
    assert_eq!(
        stored.base_url, "https://api.example.eu/v1",
        "the old server stays"
    );
}
