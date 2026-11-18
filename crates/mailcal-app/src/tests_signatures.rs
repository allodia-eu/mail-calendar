//! Signature-library tests for [`super::App`]: the library and its assignments read back and
//! surviving a relaunch, the composer resolution (mode → slot → body), and the ways an assignment
//! must never be left dangling: deleting the signature, and removing the account. What a change
//! runs and comes to is in `tests_signature_work.rs`; the shared fixtures live in
//! `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app, app_with_prefs};
use mailcal_viewmodel::SignatureSlotKind;
use tempfile::TempDir;

use super::{SignatureChange, Surface};
use crate::{App, tests_fakes as fakes};

/// A 1×1 transparent PNG, base64.
pub(super) const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// The preferences path in a data directory of the test's own (the signature library is its
/// sibling, derived by `App::new`).
pub(super) fn prefs_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("preferences.toml")
}

/// Saves a new signature, run to its end, and returns its id.
pub(super) fn create(app: &App<FakeProvider>, name: &str, body_html: &str) -> String {
    let id = app.signatures().new_id;
    save(app, &id, name, body_html);
    id
}

/// Saves `id`, run to its end.
pub(super) fn save(app: &App<FakeProvider>, id: &str, name: &str, body_html: &str) {
    app.change_signature_now(SignatureChange::Save {
        id: id.to_owned(),
        name: name.to_owned(),
        body_html: body_html.to_owned(),
        body_plain: name.to_owned(),
    });
}

/// Deletes `id`, run to its end.
pub(super) fn delete(app: &App<FakeProvider>, id: &str) {
    app.change_signature_now(SignatureChange::Delete { id: id.to_owned() });
}

#[test]
fn a_created_signature_is_listed_and_its_body_fetched_on_demand() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let id = create(&app, "Work", "<p>Alice</p>");

    assert!(surfaces.lock().unwrap().contains(&Surface::Signatures));
    let view = app.signatures();
    assert_eq!(view.library.len(), 1);
    assert_eq!(view.library[0].id, id);
    assert_eq!(view.library[0].name, "Work");
    // The next new signature is saved under an id of its own.
    assert_ne!(view.new_id, id);
    // The list carries names only; the body is a separate fetch, so a library of ten logos
    // doesn't cross the FFI to draw ten rows.
    assert_eq!(app.signature_html(&id).as_deref(), Some("<p>Alice</p>"));
    assert_eq!(app.signature_html("no-such-id"), None);
}

#[test]
fn the_library_and_the_assignments_survive_a_relaunch() {
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));

    let id = {
        let app = app_with_prefs(
            vec![account("acct-1", FakeProvider::new())],
            prefs_path(&dir),
            &surfaces,
        );
        let id = create(&app, "Work", "<p>Alice</p>");
        app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id.clone()));
        // Closed as a host closes it: what is still being saved is written first.
        app.settle_preferences();
        id
    };

    // A fresh app over the same data directory: the library file and the assignment in
    // preferences.toml are two files, and both have to come back.
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        prefs_path(&dir),
        &surfaces,
    );
    let view = app.signatures();
    assert_eq!(view.library.len(), 1);
    assert_eq!(view.accounts.len(), 1);
    assert_eq!(view.accounts[0].new_message.as_deref(), Some(id.as_str()));
    assert_eq!(view.accounts[0].reply_forward, None);
    assert_eq!(app.signature_html(&id).as_deref(), Some("<p>Alice</p>"));
}

#[test]
fn the_signature_library_is_not_written_into_the_preferences_file() {
    // The whole reason for a second file: a preference write is a read-modify-write of the
    // entire file, and a signature carries its images inline. If bodies lived in
    // preferences.toml, toggling a swipe action would rewrite a logo's bytes.
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        prefs_path(&dir),
        &surfaces,
    );

    let id = create(&app, "Work", "<p>a-very-recognisable-body</p>");
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id));

    app.settle_preferences();
    let prefs = std::fs::read_to_string(prefs_path(&dir)).expect("preferences written");
    assert!(!prefs.contains("a-very-recognisable-body"), "{prefs}");
    let library = std::fs::read_to_string(dir.path().join("signatures.toml"))
        .expect("signature library written");
    assert!(library.contains("a-very-recognisable-body"), "{library}");
}

/// The composer resolves a signature on opening and on every From change, so the assignments
/// are read from memory: a preferences file that changed on disk since launch does not move them.
#[test]
fn the_assignments_are_read_from_memory_not_from_the_file() {
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        prefs_path(&dir),
        &surfaces,
    );
    let id = create(&app, "Work", "<p>W</p>");
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id.clone()));
    app.settle_preferences();

    std::fs::write(prefs_path(&dir), "this is not toml").unwrap();

    assert_eq!(
        app.resolve_signature("acct-1", SignatureSlotKind::NewMessage)
            .map(|body| body.id),
        Some(id)
    );
}

#[test]
fn the_two_slots_resolve_independently_and_an_unassigned_slot_seeds_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let work = create(&app, "Work", "<p>Work</p>");
    let short = create(&app, "Short", "<p>D.</p>");

    // A new message gets the full signature; a reply gets the short one: the Outlook model, and
    // the reason the two slots exist rather than one flag.
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(work.clone()));
    app.assign_signature(
        "acct-1",
        SignatureSlotKind::ReplyForward,
        Some(short.clone()),
    );

    let new = app
        .resolve_signature("acct-1", SignatureSlotKind::NewMessage)
        .expect("a new-message signature");
    assert_eq!(new.id, work);
    assert_eq!(new.body_html, "<p>Work</p>");
    assert_eq!(new.body_plain, "Work");
    assert_eq!(
        app.resolve_signature("acct-1", SignatureSlotKind::ReplyForward)
            .map(|body| body.id),
        Some(short)
    );

    // Clearing one slot leaves the other alone, and an unassigned slot seeds nothing at all;
    // "None for both" is how a user turns signatures off, so it must be reachable.
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, None);
    assert!(
        app.resolve_signature("acct-1", SignatureSlotKind::NewMessage)
            .is_none()
    );
    assert!(
        app.resolve_signature("acct-1", SignatureSlotKind::ReplyForward)
            .is_some()
    );
    // An account nobody configured has no signature either way.
    assert!(
        app.resolve_signature("acct-2", SignatureSlotKind::NewMessage)
            .is_none()
    );
}

/// The no-dangling-assignment teardown holds whether or not there is a file behind it.
#[test]
fn deleting_a_signature_clears_the_accounts_that_used_it() {
    let dir = TempDir::new().unwrap();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let stored = app_with_prefs(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
        ],
        prefs_path(&dir),
        &surfaces,
    );
    let in_memory = app(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
        ],
        &surfaces,
    );
    for app in [&stored, &in_memory] {
        let doomed = create(app, "Work", "<p>W</p>");
        let kept = create(app, "Personal", "<p>P</p>");
        app.assign_signature(
            "acct-1",
            SignatureSlotKind::NewMessage,
            Some(doomed.clone()),
        );
        app.assign_signature(
            "acct-1",
            SignatureSlotKind::ReplyForward,
            Some(kept.clone()),
        );
        app.assign_signature(
            "acct-2",
            SignatureSlotKind::NewMessage,
            Some(doomed.clone()),
        );

        delete(app, &doomed);

        let view = app.signatures();
        assert_eq!(view.library.len(), 1);
        assert_eq!(view.library[0].id, kept);
        // Every slot that pointed at the deleted signature is cleared (across accounts) so no
        // assignment names something that no longer exists.
        assert_eq!(view.accounts[0].new_message, None);
        assert_eq!(
            view.accounts[0].reply_forward.as_deref(),
            Some(kept.as_str())
        );
        assert_eq!(view.accounts[1].new_message, None);
        assert!(
            app.resolve_signature("acct-2", SignatureSlotKind::NewMessage)
                .is_none()
        );
    }
    stored.settle_preferences();
    let prefs = std::fs::read_to_string(prefs_path(&dir)).unwrap();
    assert_eq!(prefs.matches("new_message").count(), 0, "{prefs}");
}

#[test]
fn assigning_a_signature_that_does_not_exist_clears_the_slot_instead() {
    // A host racing a delete (or a stale picker) must not be able to store a pointer that
    // resolves to nothing: the slot reads as "None", so that is what gets written.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = create(&app, "Work", "<p>W</p>");
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id));

    app.assign_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some("no-such-id".to_owned()),
    );

    assert_eq!(app.signatures().accounts[0].new_message, None);
    assert!(surfaces.lock().unwrap().contains(&Surface::Signatures));
}

#[tokio::test]
async fn removing_an_account_drops_its_assignment_but_keeps_the_shared_library() {
    // Signatures are standalone entities: removing the account that used one must not delete it,
    // because other accounts may use the same signature and the user may re-add this one.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let id = create(&app, "Work", "<p>W</p>");
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id));

    app.remove_account(&engine_api::AccountId::try_from("acct-1").unwrap())
        .await;

    // The library is intact…
    let view = app.signatures();
    assert_eq!(view.library.len(), 1);
    assert_eq!(view.accounts, []);
    // …and the removed account's assignment is gone, so a re-add starts with no signature rather
    // than inheriting one the user may have deleted meanwhile.
    assert!(
        app.resolve_signature("acct-1", SignatureSlotKind::NewMessage)
            .is_none()
    );
}

#[test]
fn a_stored_signature_body_is_sanitized_on_the_way_in() {
    // The library is sanitised on write, not only on send: what comes out of it is assigned into
    // the composer's editor with `innerHTML`, and that page's CSP permits inline handlers. So a
    // body reaching the editor is already inert, and `signatures.toml` is a plain file anything
    // with disk access can edit, which is exactly why this boundary exists.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let image = format!("data:image/png;base64,{PNG}");

    let id = create(
        &app,
        "Work",
        &format!("<p onclick=\"steal()\">Alice</p><script>alert(1)</script><img src=\"{image}\">"),
    );

    let html = app.signature_html(&id).expect("a stored body");
    assert!(!html.contains("script"), "{html}");
    assert!(!html.contains("onclick"), "{html}");
    // …while the inline image survives, which is what makes an embedded logo possible at all.
    assert!(html.contains(&image), "{html}");

    // An edit goes through the same gate; otherwise the second save is the way in.
    save(&app, &id, "Work", "<p onmouseover=\"steal()\">Alice</p>");
    assert!(!app.signature_html(&id).unwrap().contains("onmouseover"));
}

#[test]
fn an_assignment_survives_without_a_preferences_file() {
    // "Persistence is off" means "lives for this run", not "is silently discarded": an in-memory
    // boot (the showcase) holds a library its accounts are pointed at.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let id = create(&app, "Work", "<p>Sam</p>");
    app.assign_signature("acct-1", SignatureSlotKind::NewMessage, Some(id.clone()));

    let view = app.signatures();
    let assigned = view
        .accounts
        .iter()
        .find(|a| a.account_id == "acct-1")
        .expect("the account is listed");
    assert_eq!(assigned.new_message.as_deref(), Some(id.as_str()));
    assert_eq!(assigned.reply_forward, None);
    let body = app
        .resolve_signature("acct-1", SignatureSlotKind::NewMessage)
        .expect("the assignment resolves to a body");
    assert_eq!(body.body_html, "<p>Sam</p>");
}
