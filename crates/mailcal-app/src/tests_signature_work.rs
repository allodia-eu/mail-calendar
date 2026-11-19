//! What saving and deleting a signature runs and comes to: one change at a time, the same change
//! asked twice saved once, a refusal said with its reason, and a library that could not be
//! written raised as the failed-save notice whose Try again writes it.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app, app_with_prefs};
use mailcal_viewmodel::{
    SignatureChangeKind, SignatureImageRefusal, SignatureRefusal, SignatureResult, SignatureWork,
};
use tempfile::TempDir;

use super::SignatureChange;
use crate::{
    Surface, tests_fakes as fakes,
    tests_signatures::{PNG, create, delete, prefs_path, save},
};

fn save_change(id: &str, name: &str) -> SignatureChange {
    SignatureChange::Save {
        id: id.to_owned(),
        name: name.to_owned(),
        body_html: format!("<p>{name}</p>"),
        body_plain: name.to_owned(),
    }
}

fn result(app: &crate::App<FakeProvider>) -> SignatureResult {
    app.signatures().outcome.expect("an outcome").result
}

/// A double press on Save of a new signature saves it once: the second is the same change and
/// is dropped while the first runs, and a save under the same id once it has run changes the
/// signature the first one made rather than adding another.
#[test]
fn a_new_signature_saved_twice_is_one_signature() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = app.signatures().new_id;

    assert!(app.queue_signature_change(save_change(&id, "Work")));
    assert!(!app.queue_signature_change(save_change(&id, "Work")));
    assert_eq!(
        app.signatures().work,
        [SignatureWork {
            id: id.clone(),
            kind: SignatureChangeKind::Save
        }]
    );
    app.run_signature_changes();
    app.change_signature_now(save_change(&id, "Work"));

    let view = app.signatures();
    assert_eq!(view.library.len(), 1);
    assert_eq!(view.library[0].id, id);
    assert_eq!(view.work, []);
    let outcome = view.outcome.expect("an outcome");
    assert_eq!(outcome.id, id);
    assert_eq!(outcome.kind, SignatureChangeKind::Save);
    assert_eq!(outcome.result, SignatureResult::Saved);
}

/// A change asked for while another runs waits and runs next; each outcome is told once by its
/// serial, and the one in force is forgotten as the next change starts.
#[test]
fn a_change_asked_for_while_another_runs_waits_and_runs_next() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let kept = create(&app, "Kept", "<p>K</p>");
    let first = app.signatures().outcome.expect("an outcome").serial;
    let doomed = create(&app, "Doomed", "<p>D</p>");
    let new_id = app.signatures().new_id;

    surfaces.lock().unwrap().clear();
    assert!(app.queue_signature_change(save_change(&kept, "Kept again")));
    assert_eq!(app.signatures().outcome, None);
    assert!(!app.queue_signature_change(SignatureChange::Delete { id: doomed.clone() }));
    assert!(!app.queue_signature_change(save_change(&new_id, "New")));
    assert_eq!(
        app.signatures()
            .work
            .iter()
            .map(|work| work.kind)
            .collect::<Vec<_>>(),
        [
            SignatureChangeKind::Save,
            SignatureChangeKind::Delete,
            SignatureChangeKind::Save
        ]
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Signatures));

    app.run_signature_changes();

    let view = app.signatures();
    assert_eq!(view.work, []);
    let names: Vec<_> = view.library.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Kept again", "New"]);
    let outcome = view.outcome.expect("an outcome");
    assert_eq!(outcome.id, new_id);
    assert!(outcome.serial > first + 1);
}

#[test]
fn a_blank_name_is_refused_and_a_name_stored_without_its_spaces() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = app.signatures().new_id;

    save(&app, &id, "   ", "<p>x</p>");
    assert_eq!(
        result(&app),
        SignatureResult::Refused {
            reason: SignatureRefusal::BlankName
        }
    );
    assert_eq!(app.signatures().library, []);
    // A refused new signature keeps its id, so the editor's next press saves it.
    assert_eq!(app.signatures().new_id, id);

    save(&app, &id, "  Work ", "<p>x</p>");
    assert_eq!(app.signatures().library[0].name, "Work");
}

/// An edit never makes a signature: one deleted while it was open says so instead.
#[test]
fn an_edit_of_a_deleted_signature_is_refused_as_gone() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = create(&app, "Work", "<p>W</p>");
    delete(&app, &id);
    assert_eq!(result(&app), SignatureResult::Deleted);

    save(&app, &id, "Work", "<p>W</p>");
    assert_eq!(
        result(&app),
        SignatureResult::Refused {
            reason: SignatureRefusal::Gone
        }
    );
    save(&app, "no-such-id", "Ghost", "<p>x</p>");
    assert_eq!(app.signatures().library, []);
}

/// The save asks the same rule as the editor's picker, of the bytes it is given: an SVG, or a
/// file that claims to be a PNG and is not, is refused with the reason.
#[test]
fn a_picture_the_rule_refuses_is_not_saved() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = app.signatures().new_id;
    let svg = STANDARD.encode(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#);

    save(
        &app,
        &id,
        "Logo",
        &format!(r#"<img src="data:image/png;base64,{svg}">"#),
    );
    assert_eq!(
        result(&app),
        SignatureResult::Refused {
            reason: SignatureRefusal::Image {
                refusal: SignatureImageRefusal::Format
            }
        }
    );
    assert_eq!(app.signatures().library, []);

    save(
        &app,
        &id,
        "Logo",
        &format!(r#"<img src="data:image/png;base64,{PNG}">"#),
    );
    assert_eq!(result(&app), SignatureResult::Saved);
}

/// A library that cannot be written still holds the change for this run, says so as the
/// failed-save notice, and is written by its Try again.
#[test]
fn a_library_that_could_not_be_written_is_said_and_written_by_a_retry() {
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        prefs_path(&dir),
        &surfaces,
    );
    // A directory where the file goes: nothing can be renamed over it.
    let library = dir.path().join("signatures.toml");
    std::fs::create_dir(&library).unwrap();

    let id = create(&app, "Work", "<p>W</p>");

    assert_eq!(result(&app), SignatureResult::NotWritten);
    assert_eq!(app.signatures().library.len(), 1);
    assert!(app.preferences_not_saved());
    assert!(surfaces.lock().unwrap().contains(&Surface::Notices));

    std::fs::remove_dir(&library).unwrap();
    app.retry_preferences_save();
    assert!(!app.preferences_not_saved());
    let written = std::fs::read_to_string(&library).unwrap();
    assert!(written.contains(&id), "{written}");
}

/// Closing waits for the library as it waits for the preferences.
#[test]
fn closing_waits_for_the_library_to_be_written() {
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        prefs_path(&dir),
        &surfaces,
    );
    let id = app.signatures().new_id;
    assert!(app.queue_signature_change(save_change(&id, "Work")));
    app.run_signature_changes();

    assert!(app.settle_preferences_within(std::time::Duration::from_secs(5)));
    let written = std::fs::read_to_string(dir.path().join("signatures.toml")).unwrap();
    assert!(written.contains("Work"), "{written}");
}

/// A signature body is the person's own writing, and a change is never printed with it.
#[test]
fn a_change_prints_without_its_body() {
    let printed = format!(
        "{:?}",
        SignatureChange::Save {
            id: "abc".to_owned(),
            name: "Private name".to_owned(),
            body_html: "<p>+31 6 1234</p>".to_owned(),
            body_plain: "+31 6 1234".to_owned(),
        }
    );
    assert!(printed.contains("abc"), "{printed}");
    assert!(!printed.contains("1234"), "{printed}");
    assert!(!printed.contains("Private"), "{printed}");
}
