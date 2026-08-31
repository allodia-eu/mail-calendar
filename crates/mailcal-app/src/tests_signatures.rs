//! Signature-library tests for [`super::App`]: the CRUD round-trip, per-account assignment, the
//! composer resolution (mode → slot → body), and the two ways an assignment must never be left
//! dangling; deleting the signature, and removing the account. The shared fixtures live in
//! `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app, app_with_prefs};
use mailcal_viewmodel::SignatureSlotKind;

use super::Surface;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A fresh, empty app-data directory for one test, and the preferences path inside it (the
/// signature library is its sibling, derived by `App::new`).
fn prefs_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("preferences.toml")
}

#[tokio::test]
async fn a_created_signature_is_listed_and_its_body_fetched_on_demand() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let row = app
        .create_signature(
            "Work".to_owned(),
            "<p>Alice</p>".to_owned(),
            "Alice".to_owned(),
        )
        .await;

    // The create returns the minted id, so a host can select what it just made without
    // re-pulling and guessing which row is new.
    assert!(!row.id.is_empty());
    assert_eq!(row.name, "Work");
    assert!(surfaces.lock().unwrap().contains(&Surface::Settings));

    let snapshot = app.signatures().await;
    assert_eq!(snapshot.signatures.len(), 1);
    assert_eq!(snapshot.signatures[0].id, row.id);
    // The list carries names only; the body is a separate fetch, so a library of ten logos
    // doesn't cross the FFI to draw ten rows.
    assert_eq!(app.signature_html(&row.id).as_deref(), Some("<p>Alice</p>"));
    assert_eq!(app.signature_html("no-such-id"), None);
}

#[tokio::test]
async fn an_update_replaces_the_body_but_an_unknown_id_creates_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    let row = app
        .create_signature("Work".to_owned(), "<p>old</p>".to_owned(), "old".to_owned())
        .await;

    assert!(
        app.update_signature(
            &row.id,
            "Work (new)".to_owned(),
            "<p>new</p>".to_owned(),
            "new".to_owned(),
        )
        .await
    );
    assert_eq!(app.signature_html(&row.id).as_deref(), Some("<p>new</p>"));
    assert_eq!(app.signatures().await.signatures[0].name, "Work (new)");

    // An id that names nothing must not silently create a signature the user thought they were
    // editing; they would then have two, one of them invisible to the edit they just made.
    assert!(
        !app.update_signature(
            "no-such-id",
            "Ghost".to_owned(),
            "<p>x</p>".to_owned(),
            "x".to_owned(),
        )
        .await
    );
    assert_eq!(app.signatures().await.signatures.len(), 1);
}

#[tokio::test]
async fn the_library_and_the_assignments_survive_a_relaunch() {
    let path = prefs_path("mailcal-signatures-relaunch-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));

    let id = {
        let app = app_with_prefs(
            vec![account("acct-1", FakeProvider::new())],
            path.clone(),
            &surfaces,
        );
        let row = app
            .create_signature(
                "Work".to_owned(),
                "<p>Alice</p>".to_owned(),
                "Alice".to_owned(),
            )
            .await;
        app.set_account_signature(
            "acct-1",
            SignatureSlotKind::NewMessage,
            Some(row.id.clone()),
        )
        .await;
        row.id
    };

    // A fresh app over the same data directory: the library file and the assignment in
    // preferences.toml are two files, and both have to come back.
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );
    let snapshot = app.signatures().await;
    assert_eq!(snapshot.signatures.len(), 1);
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(
        snapshot.accounts[0].new_message.as_deref(),
        Some(id.as_str())
    );
    assert_eq!(snapshot.accounts[0].reply_forward, None);
    assert_eq!(app.signature_html(&id).as_deref(), Some("<p>Alice</p>"));

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn the_signature_library_is_not_written_into_the_preferences_file() {
    // The whole reason for a second file: a preference write is a read-modify-write of the
    // entire file, and a signature carries its images inline. If bodies lived in
    // preferences.toml, toggling a swipe action would rewrite a logo's bytes.
    let path = prefs_path("mailcal-signatures-separate-file-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );

    app.create_signature(
        "Work".to_owned(),
        "<p>a-very-recognisable-body</p>".to_owned(),
        "body".to_owned(),
    )
    .await;

    let prefs = std::fs::read_to_string(&path).expect("preferences written");
    assert!(!prefs.contains("a-very-recognisable-body"), "{prefs}");
    let library = std::fs::read_to_string(path.parent().unwrap().join("signatures.toml"))
        .expect("signature library written");
    assert!(library.contains("a-very-recognisable-body"), "{library}");

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn the_two_slots_resolve_independently_and_an_unassigned_slot_seeds_nothing() {
    let path = prefs_path("mailcal-signatures-slots-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );
    let work = app
        .create_signature(
            "Work".to_owned(),
            "<p>Work</p>".to_owned(),
            "Work".to_owned(),
        )
        .await;
    let short = app
        .create_signature("Short".to_owned(), "<p>D.</p>".to_owned(), "D.".to_owned())
        .await;

    // A new message gets the full signature; a reply gets the short one: the Outlook model, and
    // the reason the two slots exist rather than one flag.
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some(work.id.clone()),
    )
    .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::ReplyForward,
        Some(short.id.clone()),
    )
    .await;

    let new = app
        .resolve_signature("acct-1", SignatureSlotKind::NewMessage)
        .expect("a new-message signature");
    assert_eq!(new.id, work.id);
    assert_eq!(new.body_html, "<p>Work</p>");
    assert_eq!(new.body_plain, "Work");
    assert_eq!(
        app.resolve_signature("acct-1", SignatureSlotKind::ReplyForward)
            .map(|body| body.id),
        Some(short.id)
    );

    // Clearing one slot leaves the other alone, and an unassigned slot seeds nothing at all;
    // "None for both" is how a user turns signatures off, so it must be reachable.
    app.set_account_signature("acct-1", SignatureSlotKind::NewMessage, None)
        .await;
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

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn deleting_a_signature_clears_the_accounts_that_used_it() {
    let path = prefs_path("mailcal-signatures-delete-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
        ],
        path.clone(),
        &surfaces,
    );
    let doomed = app
        .create_signature("Work".to_owned(), "<p>W</p>".to_owned(), "W".to_owned())
        .await;
    let kept = app
        .create_signature("Personal".to_owned(), "<p>P</p>".to_owned(), "P".to_owned())
        .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some(doomed.id.clone()),
    )
    .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::ReplyForward,
        Some(kept.id.clone()),
    )
    .await;
    app.set_account_signature(
        "acct-2",
        SignatureSlotKind::NewMessage,
        Some(doomed.id.clone()),
    )
    .await;

    assert!(app.delete_signature(&doomed.id).await);
    assert!(!app.delete_signature(&doomed.id).await);

    let snapshot = app.signatures().await;
    assert_eq!(snapshot.signatures.len(), 1);
    assert_eq!(snapshot.signatures[0].id, kept.id);
    // Every slot that pointed at the deleted signature is cleared (across accounts) so no
    // assignment names something that no longer exists.
    let acct_1 = &snapshot.accounts[0];
    assert_eq!(acct_1.new_message, None);
    assert_eq!(acct_1.reply_forward.as_deref(), Some(kept.id.as_str()));
    assert_eq!(snapshot.accounts[1].new_message, None);
    assert!(
        app.resolve_signature("acct-2", SignatureSlotKind::NewMessage)
            .is_none()
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn assigning_a_signature_that_does_not_exist_clears_the_slot_instead() {
    // A host racing a delete (or a stale picker) must not be able to store a pointer that
    // resolves to nothing: the slot reads as "None", so that is what gets written.
    let path = prefs_path("mailcal-signatures-unknown-assign-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );

    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some("no-such-id".to_owned()),
    )
    .await;

    assert_eq!(app.signatures().await.accounts[0].new_message, None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn removing_an_account_drops_its_assignment_but_keeps_the_shared_library() {
    // Signatures are standalone entities: removing the account that used one must not delete it,
    // because other accounts may use the same signature and the user may re-add this one.
    let path = prefs_path("mailcal-signatures-account-removal-test");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct-1", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );
    let row = app
        .create_signature("Work".to_owned(), "<p>W</p>".to_owned(), "W".to_owned())
        .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some(row.id.clone()),
    )
    .await;

    app.remove_account(&engine_api::AccountId::try_from("acct-1").unwrap())
        .await;

    // The library is intact…
    let snapshot = app.signatures().await;
    assert_eq!(snapshot.signatures.len(), 1);
    assert!(snapshot.accounts.is_empty());
    // …and the removed account's assignment is gone, so a re-add starts with no signature rather
    // than inheriting one the user may have deleted meanwhile.
    assert!(
        app.resolve_signature("acct-1", SignatureSlotKind::NewMessage)
            .is_none()
    );

    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[tokio::test]
async fn a_stored_signature_body_is_sanitized_on_the_way_in() {
    // The library is sanitised on write, not only on send: what comes out of it is assigned into
    // the composer's editor with `innerHTML`, and that page's CSP permits inline handlers. So a
    // body reaching the editor is already inert, and `signatures.toml` is a plain file anything
    // with disk access can edit, which is exactly why this boundary exists.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let row = app
        .create_signature(
            "Work".to_owned(),
            "<p onclick=\"steal()\">Alice</p><script>alert(1)</script>\
             <img src=\"data:image/png;base64,AAAA\">"
                .to_owned(),
            "Alice".to_owned(),
        )
        .await;

    let html = app.signature_html(&row.id).expect("a stored body");
    assert!(!html.contains("script"), "{html}");
    assert!(!html.contains("onclick"), "{html}");
    // …while the inline image survives, which is what makes an embedded logo possible at all.
    assert!(html.contains("data:image/png;base64,AAAA"), "{html}");

    // An edit goes through the same gate; otherwise the second save is the way in.
    app.update_signature(
        &row.id,
        "Work".to_owned(),
        "<p onmouseover=\"steal()\">Alice</p>".to_owned(),
        "Alice".to_owned(),
    )
    .await;
    assert!(!app.signature_html(&row.id).unwrap().contains("onmouseover"));
}

#[tokio::test]
async fn an_assignment_survives_without_a_preferences_file() {
    // "Persistence is off" must mean "lives for this run", not "is silently discarded". The
    // library already behaved that way (its writes no-op over an in-memory `Signatures`) but the
    // per-account assignment lived ONLY in preferences.toml, so with no path it vanished the
    // instant it was made. An in-memory boot could therefore hold a library that no account could
    // be pointed at, which is precisely what the showcase screenshots hit: a seeded set of
    // signatures, and a Settings screen reporting every slot as None.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let row = app
        .create_signature("Work".to_owned(), "<p>Sam</p>".to_owned(), "Sam".to_owned())
        .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::NewMessage,
        Some(row.id.clone()),
    )
    .await;

    // It reads back on the settings surface...
    let snapshot = app.signatures().await;
    let assigned = snapshot
        .accounts
        .iter()
        .find(|a| a.account_id == "acct-1")
        .expect("the account is listed");
    assert_eq!(assigned.new_message.as_deref(), Some(row.id.as_str()));
    // ...and, the half that actually reaches a message, it resolves to a body the composer seeds.
    let body = app
        .resolve_signature("acct-1", SignatureSlotKind::NewMessage)
        .expect("the assignment resolves to a body");
    assert_eq!(body.body_html, "<p>Sam</p>");
    // The other slot is independent, so it stays unset.
    assert_eq!(assigned.reply_forward, None);
}

#[tokio::test]
async fn deleting_a_signature_clears_an_in_memory_assignment_too() {
    // The no-dangling-assignment teardown has to hold on both storage paths, or an in-memory boot
    // keeps a slot pointing at a signature that no longer exists.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    let row = app
        .create_signature("Work".to_owned(), "<p>Sam</p>".to_owned(), "Sam".to_owned())
        .await;
    app.set_account_signature(
        "acct-1",
        SignatureSlotKind::ReplyForward,
        Some(row.id.clone()),
    )
    .await;
    assert!(app.delete_signature(&row.id).await);

    let snapshot = app.signatures().await;
    let account_row = snapshot
        .accounts
        .iter()
        .find(|a| a.account_id == "acct-1")
        .expect("the account is listed");
    assert_eq!(account_row.reply_forward, None, "the slot was cleared");
    assert!(
        app.resolve_signature("acct-1", SignatureSlotKind::ReplyForward)
            .is_none()
    );
}
