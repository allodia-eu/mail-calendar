//! The own endpoint's form (`docs/ai.md` rules 6 and 7): every gap said at once by the function the
//! save refuses by, Save and Remove running until written, the key kept in the keystore and never
//! drawn back, and a refusal or a failed write said on the form.

use mailcal_ai::{Class, Mode, Refused};

use super::{
    AiEndpointAddressGap, AiEndpointChange, AiEndpointChangeKind, AiEndpointDeclarationGap,
    AiEndpointEdit, AiEndpointGaps, AiEndpointResult, BuildKind, ai_endpoint_gaps,
    tests_support::Fixture,
};
use crate::Surface;

fn edit(address: &str, model: &str, declared: Option<Class>, key: &str) -> AiEndpointEdit {
    AiEndpointEdit {
        address: address.to_owned(),
        model: model.to_owned(),
        declared,
        key: key.to_owned(),
    }
}

fn result(fixture: &Fixture) -> AiEndpointResult {
    fixture
        .app
        .ai_endpoint()
        .outcome
        .expect("an outcome")
        .result
}

/// While not offered a change to the endpoint is refused before it is queued: nothing reaches the
/// preferences or the keystore, nothing is signalled and no outcome waits for a form nobody draws
/// (rule 4). An endpoint already stored is kept.
#[test]
fn a_change_is_refused_while_the_assistant_is_not_offered() {
    let fixture = Fixture::new(BuildKind::Production);
    let save = AiEndpointChange::Save(edit(
        "https://models.example.eu/v1",
        "m",
        Some(Class::EuNative),
        "sk-test",
    ));
    for change in [save.clone(), AiEndpointChange::Remove] {
        assert!(!fixture.app.queue_ai_endpoint_change(change.clone()));
        fixture.app.change_ai_endpoint_now(change);
    }
    assert!(!fixture.signalled(Surface::AiEndpoint));
    let view = fixture.app.ai_endpoint();
    assert!(!view.offered);
    assert!(!view.stored);
    assert_eq!(view.outcome, None);
    assert!(view.work.is_empty());
    assert!(fixture.host.seen.lock().unwrap().keys.is_empty());

    fixture
        .app
        .set_ai_entitlement(super::AiEntitlement::Granted);
    fixture.app.change_ai_endpoint_now(save);
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    fixture
        .app
        .set_ai_entitlement(super::AiEntitlement::NotGranted);
    fixture.app.change_ai_endpoint_now(AiEndpointChange::Remove);
    assert!(fixture.app.ai_endpoint().stored, "kept while not offered");
}

#[test]
fn every_field_is_judged_at_once() {
    assert_eq!(
        ai_endpoint_gaps("", " ", None, Mode::EuNative),
        AiEndpointGaps {
            address: Some(AiEndpointAddressGap::Missing),
            model_missing: true,
            declaration: Some(AiEndpointDeclarationGap::Missing),
            kept_key: None,
        }
    );
    assert_eq!(
        ai_endpoint_gaps(
            "http://models.example.eu/v1",
            "m",
            Some(Class::EuHosted),
            Mode::EuNative
        ),
        AiEndpointGaps {
            address: Some(AiEndpointAddressGap::NeedsHttps),
            model_missing: false,
            declaration: Some(AiEndpointDeclarationGap::Refused(Refused::EuHosted)),
            kept_key: None,
        }
    );
    assert_eq!(
        ai_endpoint_gaps("not a url", "m", Some(Class::NonEu), Mode::All).address,
        Some(AiEndpointAddressGap::Unusable)
    );
    let fine = ai_endpoint_gaps(
        "http://127.0.0.1:11434/v1",
        "m",
        Some(Class::EuHosted),
        Mode::EuHosted,
    );
    assert!(fine.is_empty(), "{fine:?}");
}

#[test]
fn a_save_refuses_what_the_form_said_and_keeps_the_edit_on_the_form() {
    let fixture = Fixture::new(BuildKind::Development);
    let refused = edit("http://models.example.eu/v1", "", Some(Class::NonEu), "sk");
    let predicted = fixture
        .app
        .ai_endpoint_gap(&refused.address, &refused.model, refused.declared);
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(refused));

    assert_eq!(
        result(&fixture),
        AiEndpointResult::Refused { gaps: predicted },
        "the save refuses by the function the form asks"
    );
    let view = fixture.app.ai_endpoint();
    assert_eq!(view.fields.address, "http://models.example.eu/v1");
    assert_eq!(view.fields.declared, Some(Class::NonEu));
    assert!(!view.stored);
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [],
        "nothing reaches the keystore from a refused save"
    );
}

#[test]
fn a_save_keeps_the_key_in_the_keystore_trimmed_and_never_draws_it_back() {
    let fixture = Fixture::new(BuildKind::Development);
    assert!(
        fixture
            .app
            .queue_ai_endpoint_change(AiEndpointChange::Save(edit(
                " https://models.example.eu/v1/ ",
                " big-model ",
                Some(Class::EuNative),
                "  sk-secret  ",
            )))
    );
    assert_eq!(fixture.app.ai_endpoint().work, [AiEndpointChangeKind::Save]);
    assert!(fixture.signalled(Surface::AiEndpoint));
    fixture.app.run_ai_endpoint_changes();

    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    let view = fixture.app.ai_endpoint();
    assert_eq!(view.work, []);
    assert!(view.stored);
    assert_eq!(view.fields.address, "https://models.example.eu/v1");
    assert_eq!(view.fields.model, "big-model");
    assert!(view.fields.key_kept);
    assert!(!format!("{view:?}").contains("sk-secret"));
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk-secret".to_owned())]
    );

    // An edit that leaves the key alone does not touch the keystore.
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v2",
            "big-model",
            Some(Class::EuNative),
            "",
        )));
    assert_eq!(fixture.host.seen.lock().unwrap().keys.len(), 1);
    assert!(fixture.app.ai_endpoint().fields.key_kept);
    let outcome = fixture.app.ai_endpoint().outcome.unwrap();
    assert_eq!(outcome.serial, 2, "each change has an outcome of its own");
}

#[test]
fn a_key_the_keystore_refused_is_said_and_nothing_changes() {
    let fixture = Fixture::new(BuildKind::Development);
    *fixture.host.refuse_keys.lock().unwrap() = true;
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "sk",
        )));
    assert_eq!(result(&fixture), AiEndpointResult::KeyNotStored);
    let view = fixture.app.ai_endpoint();
    assert!(!view.stored);
    assert_eq!(view.fields.address, "https://models.example.eu/v1");
    assert!(!view.fields.key_kept);
}

#[test]
fn an_empty_key_writes_nothing_to_the_keystore() {
    let fixture = Fixture::new(BuildKind::Development);
    *fixture.host.refuse_keys.lock().unwrap() = true;
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "  ",
        )));
    assert_eq!(
        result(&fixture),
        AiEndpointResult::Saved,
        "a keystore that refuses everything is never asked"
    );
    assert!(!fixture.app.ai_endpoint().fields.key_kept);

    *fixture.host.refuse_keys.lock().unwrap() = false;
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "sk",
        )));
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v2",
            "m",
            Some(Class::EuNative),
            "",
        )));
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    assert!(
        fixture.app.ai_endpoint().fields.key_kept,
        "an empty field keeps the key"
    );
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk".to_owned())]
    );
}

#[test]
fn remove_is_offered_while_an_endpoint_is_stored() {
    let fixture = Fixture::new(BuildKind::Development);
    assert!(!fixture.app.ai_endpoint().removable, "nothing to remove");

    fixture.app.restore_ai_endpoint_key(Some("sk".to_owned()));
    assert!(
        !fixture.app.ai_endpoint().removable,
        "a key alone is taken out at launch"
    );

    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "",
        )));
    assert!(fixture.app.ai_endpoint().removable, "an endpoint alone");

    assert!(
        fixture
            .app
            .queue_ai_endpoint_change(AiEndpointChange::Remove)
    );
    assert!(
        fixture.app.ai_endpoint().removable,
        "offered while its removal runs"
    );
    fixture.app.run_ai_endpoint_changes();
    assert!(!fixture.app.ai_endpoint().removable);
}

#[test]
fn remove_takes_the_key_out_with_the_endpoint() {
    let fixture = Fixture::new(BuildKind::Development);
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "sk",
        )));
    fixture.app.change_ai_endpoint_now(AiEndpointChange::Remove);
    assert_eq!(result(&fixture), AiEndpointResult::Removed);
    let view = fixture.app.ai_endpoint();
    assert!(!view.stored);
    assert!(!view.fields.key_kept);
    assert_eq!(view.fields.address, "");
    assert_eq!(
        fixture.host.seen.lock().unwrap().keys,
        [Some("sk".to_owned()), None]
    );
}

#[test]
fn the_same_change_asked_twice_runs_once() {
    let fixture = Fixture::new(BuildKind::Development);
    let change = AiEndpointChange::Save(edit(
        "https://models.example.eu/v1",
        "m",
        Some(Class::EuNative),
        "sk",
    ));
    assert!(fixture.app.queue_ai_endpoint_change(change.clone()));
    assert!(!fixture.app.queue_ai_endpoint_change(change));
    assert!(
        !fixture
            .app
            .queue_ai_endpoint_change(AiEndpointChange::Remove)
    );
    assert_eq!(
        fixture.app.ai_endpoint().work,
        [AiEndpointChangeKind::Save, AiEndpointChangeKind::Remove]
    );
    fixture.app.run_ai_endpoint_changes();
    assert_eq!(fixture.host.seen.lock().unwrap().keys.len(), 2);
    assert_eq!(result(&fixture), AiEndpointResult::Removed);
}

#[test]
fn a_saved_endpoint_survives_a_relaunch_and_its_key_is_restored_from_the_keystore() {
    let mut fixture = Fixture::stored(BuildKind::Development, |_| {});
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "sk",
        )));
    assert_eq!(result(&fixture), AiEndpointResult::Saved);
    let dir = fixture.dir.take().unwrap();
    drop(fixture);
    let relaunched = Fixture::reopened(dir, BuildKind::Development);
    relaunched
        .app
        .restore_ai_endpoint_key(Some("sk".to_owned()));
    let view = relaunched.app.ai_endpoint();
    assert!(view.stored);
    assert_eq!(view.fields.declared, Some(Class::EuNative));
    assert!(view.fields.key_kept);
}

#[test]
fn a_save_the_preferences_could_not_take_is_said_as_the_notice() {
    let fixture = Fixture::stored(BuildKind::Development, |dir| {
        // A directory where the file goes: nothing can be renamed over it.
        std::fs::create_dir(dir.join("preferences.toml")).unwrap();
    });
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(edit(
            "https://models.example.eu/v1",
            "m",
            Some(Class::EuNative),
            "",
        )));
    assert_eq!(result(&fixture), AiEndpointResult::NotWritten);
    assert!(
        fixture.app.ai_endpoint().stored,
        "held until the app closes"
    );
    assert!(fixture.app.preferences_not_saved());
    assert!(fixture.signalled(Surface::Notices));
}
