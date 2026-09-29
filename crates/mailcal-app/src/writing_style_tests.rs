//! The writing-style library: create by learning, rename, notes, assign, resolve, forget, and the
//! teardown paths that leave no assignment dangling. The learning and drafting runs themselves are
//! in `writing_style_ai_tests.rs`.

use std::sync::{Arc, Mutex};

use mailcal_account::StoredWritingStyle;
use mailcal_ai::{LanguageStyle, StyleGuide};

use crate::Surface;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[allow(clippy::duplicate_mod)]
#[path = "writing_style_ai_tests.rs"]
mod ai;

fn stored(name: &str) -> StoredWritingStyle {
    let mut guide = StyleGuide::new();
    guide
        .languages
        .insert("nl".to_owned(), LanguageStyle::default());
    StoredWritingStyle {
        name: name.to_owned(),
        source: "acct-1".to_owned(),
        guide_json: serde_json::to_string(&guide).unwrap(),
        exemplars_json: String::new(),
    }
}

fn app_at(dir: &std::path::Path) -> (crate::App<fakes::FakeProvider>, Arc<Mutex<Vec<Surface>>>) {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = fakes::app_with_prefs(
        vec![fakes::account("acct-1", fakes::FakeProvider::new())],
        dir.join("preferences.toml"),
        &surfaces,
    );
    (app, surfaces)
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-writing-style-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn a_learned_style_is_assigned_to_its_account_when_it_had_none_and_survives_a_relaunch() {
    let dir = temp("relaunch");
    let (app, _) = app_at(&dir);
    let first = app.store_writing_style("acct-1", stored("Work"));
    let second = app.store_writing_style("acct-1", stored("Home"));

    // The first style took the empty slot; the second did not displace it.
    assert_eq!(
        app.resolve_writing_style("acct-1").as_deref(),
        Some(first.as_str())
    );

    let (relaunched, _) = app_at(&dir);
    let snapshot = relaunched.writing_styles().await;
    let names: Vec<&str> = snapshot
        .styles
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(names, ["Work", "Home"]);
    assert_eq!(snapshot.styles[1].id, second.as_str());
    assert_eq!(snapshot.styles[0].languages, ["nl"]);
    assert_eq!(snapshot.accounts[0].style.as_deref(), Some(first.as_str()));
    assert!(snapshot.route.is_none(), "no backend was installed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn forgetting_a_style_clears_every_account_that_drafted_in_it() {
    let dir = temp("forget");
    let (app, surfaces) = app_at(&dir);
    let id = app.store_writing_style("acct-1", stored("Work"));
    app.set_account_writing_style("acct-2", Some(id.as_str().to_owned()));

    assert!(app.delete_writing_style(id.as_str()));
    assert!(!app.delete_writing_style(id.as_str()));

    assert_eq!(app.resolve_writing_style("acct-1"), None);
    let prefs = mailcal_account::load_preferences(dir.join("preferences.toml"));
    assert!(prefs.ai.writing_styles.is_empty());
    assert!(surfaces.lock().unwrap().contains(&Surface::WritingStyle));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_unknown_style_clears_the_slot_rather_than_pointing_at_nothing() {
    let dir = temp("unknown");
    let (app, _) = app_at(&dir);
    let id = app.store_writing_style("acct-1", stored("Work"));
    app.set_account_writing_style("acct-1", Some("no-such-style".to_owned()));
    assert_eq!(app.resolve_writing_style("acct-1"), None);
    app.set_account_writing_style("acct-1", Some(id.as_str().to_owned()));
    app.set_account_writing_style("acct-1", None);
    assert_eq!(app.resolve_writing_style("acct-1"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn removing_an_account_drops_its_writing_style() {
    let dir = temp("remove-account");
    let (app, _) = app_at(&dir);
    app.store_writing_style("acct-1", stored("Work"));
    app.remove_account_writing_style("acct-1");
    assert_eq!(app.resolve_writing_style("acct-1"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_rename_and_the_notes_are_kept_and_the_guide_s_other_fields_survive_them() {
    let dir = temp("edit");
    let (app, _) = app_at(&dir);
    let id = app.store_writing_style("acct-1", stored("Work"));

    assert!(app.rename_writing_style(id.as_str(), "Office".to_owned()));
    assert!(app.update_writing_style_notes(id.as_str(), "Never use exclamation marks.".to_owned()));
    assert!(!app.rename_writing_style("missing", "x".to_owned()));

    let detail = app.writing_style_detail(id.as_str()).unwrap();
    assert_eq!(detail.row.name, "Office");
    assert_eq!(detail.notes, "Never use exclamation marks.");
    assert_eq!(detail.languages[0].language, "nl");
    let _ = std::fs::remove_dir_all(&dir);
}

const PASSAGES: &str = r#"{"schema_version":1,"languages":{"nl":["Beste Anna, dank je."]}}"#;

/// What another device may be sent: the name and the guide. A guide that does not read is left
/// out rather than sent as an empty one, which would empty the style on every other device.
#[tokio::test]
async fn only_a_style_s_name_and_guide_are_offered_for_syncing() {
    let dir = temp("syncable");
    let (app, _) = app_at(&dir);
    let mut with_passages = stored("Work");
    with_passages.exemplars_json = PASSAGES.to_owned();
    let id = app.store_writing_style("acct-1", with_passages);
    let mut unreadable = stored("Broken");
    unreadable.guide_json = "not json".to_owned();
    app.store_writing_style("acct-1", unreadable);

    let syncable = app.syncable_writing_styles();

    assert_eq!(syncable.len(), 1);
    assert_eq!(syncable[0].id, id.as_str());
    assert_eq!(syncable[0].name, "Work");
    assert!(syncable[0].guide.languages.contains_key("nl"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_synced_name_and_guide_replace_this_device_s_and_its_passages_and_slot_stay() {
    let dir = temp("apply-synced");
    let (app, surfaces) = app_at(&dir);
    let mut with_passages = stored("Work");
    with_passages.exemplars_json = PASSAGES.to_owned();
    let id = app.store_writing_style("acct-1", with_passages);
    let mut guide = StyleGuide::new();
    guide
        .languages
        .insert("en".to_owned(), LanguageStyle::default());
    guide.notes = "Never before nine.".to_owned();

    assert!(app.apply_synced_writing_style(id.as_str(), "Office".to_owned(), &guide));
    assert!(!app.apply_synced_writing_style("missing", "x".to_owned(), &guide));

    let detail = app.writing_style_detail(id.as_str()).unwrap();
    assert_eq!(detail.row.name, "Office");
    assert_eq!(detail.notes, "Never before nine.");
    let library = mailcal_account::load_writing_styles(mailcal_account::writing_styles_path(&dir));
    assert_eq!(library.get(&id).unwrap().exemplars_json, PASSAGES);
    assert_eq!(
        app.resolve_writing_style("acct-1").as_deref(),
        Some(id.as_str())
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::WritingStyle));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn without_a_preferences_file_an_assignment_still_holds_for_the_run() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = fakes::app(
        vec![fakes::account("acct-1", fakes::FakeProvider::new())],
        &surfaces,
    );
    let id = app.store_writing_style("acct-1", stored("Work"));
    assert_eq!(
        app.resolve_writing_style("acct-1").as_deref(),
        Some(id.as_str())
    );
}
