//! The style library, the slots and the drafted replies (`docs/ai.md`, "What a style is" and
//! rule 25): the rules each change keeps, a relaunch reading them back, and a library that could
//! not be written raised as the failed-save notice.

use engine_api::MessageIdHeader;

use super::{BuildKind, StoredWritingStyle, StylesWritten, WritingStyleId, tests_support::Fixture};
use crate::Surface;

fn style(name: &str) -> StoredWritingStyle {
    StoredWritingStyle {
        name: name.to_owned(),
        learned_from: "me@acct-1.local".to_owned(),
        guide_json: "{}".to_owned(),
        exemplars_json: "{}".to_owned(),
        revealed: false,
    }
}

fn message(id: &str) -> MessageIdHeader {
    MessageIdHeader::new(id).unwrap()
}

#[test]
fn a_learned_style_takes_its_accounts_slot_only_when_that_is_empty() {
    let fixture = Fixture::new(BuildKind::Development);
    let app = &fixture.app;
    let first = app.add_writing_style(Some("acct-1"), style("Work"));
    assert!(fixture.signalled(Surface::WritingStyle));
    assert_eq!(app.writing_style_slot("acct-1"), Some(first.clone()));

    let second = app.add_writing_style(Some("acct-1"), style("Home"));
    assert_ne!(first, second, "each style has an id of its own");
    assert_eq!(app.writing_style_slot("acct-1"), Some(first));
    let names: Vec<String> = app
        .writing_styles()
        .into_iter()
        .map(|(_, style)| style.name)
        .collect();
    assert_eq!(names, ["Work", "Home"]);
}

#[test]
fn forgetting_a_style_clears_every_slot_holding_it() {
    let fixture = Fixture::new(BuildKind::Development);
    let app = &fixture.app;
    let id = app.add_writing_style(Some("acct-1"), style("Work"));
    app.assign_writing_style("acct-2", Some(id.clone()));
    assert!(app.forget_writing_style(&id));
    assert!(!app.forget_writing_style(&id));
    assert_eq!(app.writing_style_slot("acct-1"), None);
    assert_eq!(app.writing_style_slot("acct-2"), None);
    assert_eq!(app.writing_style(&id), None);
}

#[test]
fn a_slot_given_a_style_that_names_nothing_is_cleared() {
    let fixture = Fixture::new(BuildKind::Development);
    let app = &fixture.app;
    app.add_writing_style(Some("acct-1"), style("Work"));
    app.assign_writing_style("acct-1", WritingStyleId::new("nothing"));
    assert_eq!(app.writing_style_slot("acct-1"), None);
}

#[test]
fn a_replacement_never_makes_a_style_that_has_gone() {
    let fixture = Fixture::new(BuildKind::Development);
    let app = &fixture.app;
    let id = app.add_writing_style(None, style("Work"));
    let mut renamed = style("Office");
    renamed.revealed = true;
    assert!(app.replace_writing_style(&id, renamed.clone()));
    assert_eq!(app.writing_style(&id), Some(renamed));
    assert!(!app.replace_writing_style(&WritingStyleId::new("gone").unwrap(), style("x")));
    assert_eq!(app.writing_styles().len(), 1);
}

/// Once offered, an install names the drafted-reply keyword on Gmail at every launch after.
#[test]
fn an_install_the_feature_reached_names_the_keyword_for_good() {
    let mut fixture = Fixture::stored(BuildKind::Production, |_| {});
    assert!(!fixture.app.gmail_keyword_named());
    fixture.app.note_gmail_keyword_named();
    assert!(fixture.app.gmail_keyword_named());
    fixture.app.settle_preferences();
    let dir = fixture.dir.take().unwrap();
    drop(fixture);

    let relaunched = Fixture::reopened(dir, BuildKind::Production);
    assert!(relaunched.app.gmail_keyword_named());
}

#[test]
fn the_library_the_slots_and_the_drafted_replies_survive_a_relaunch() {
    let mut fixture = Fixture::stored(BuildKind::Development, |_| {});
    let id = fixture.app.add_writing_style(Some("acct-1"), style("Work"));
    fixture
        .app
        .note_drafted_reply("acct-1", message("reply@example.eu"));
    assert_eq!(fixture.app.settle_writing_styles(), StylesWritten::Written);
    fixture.app.settle_preferences();
    let dir = fixture.dir.take().unwrap();
    drop(fixture);

    let relaunched = Fixture::reopened(dir, BuildKind::Development);
    assert_eq!(relaunched.app.writing_style(&id), Some(style("Work")));
    assert_eq!(relaunched.app.writing_style_slot("acct-1"), Some(id));
    assert!(
        relaunched
            .app
            .is_drafted_reply(&message("reply@example.eu"))
    );
    assert!(!relaunched.app.is_drafted_reply(&message("mine@example.eu")));
}

#[test]
fn removing_an_account_drops_its_slot_and_its_drafted_replies() {
    let fixture = Fixture::new(BuildKind::Development);
    let app = &fixture.app;
    let id = app.add_writing_style(Some("acct-1"), style("Work"));
    app.note_drafted_reply("acct-1", message("one@example.eu"));
    app.note_drafted_reply("acct-2", message("two@example.eu"));
    app.forget_account_writing_assistant("acct-1");
    assert_eq!(app.writing_style_slot("acct-1"), None);
    assert_eq!(
        app.writing_style(&id),
        Some(style("Work")),
        "the style is standalone"
    );
    assert!(!app.is_drafted_reply(&message("one@example.eu")));
    assert!(app.is_drafted_reply(&message("two@example.eu")));
}

#[test]
fn a_library_that_could_not_be_written_is_said_and_written_by_a_retry() {
    let fixture = Fixture::stored(BuildKind::Development, |dir| {
        std::fs::create_dir(dir.join("writing_styles.toml")).unwrap();
    });
    let dir = fixture.dir.as_ref().unwrap().path().to_owned();
    let id = fixture.app.add_writing_style(None, style("Work"));
    assert_eq!(
        fixture.app.settle_writing_styles(),
        StylesWritten::NotWritten
    );
    assert!(fixture.app.preferences_not_saved());
    assert!(fixture.signalled(Surface::Notices));
    assert_eq!(
        fixture.app.writing_styles().len(),
        1,
        "held until the app closes"
    );

    std::fs::remove_dir(dir.join("writing_styles.toml")).unwrap();
    fixture.app.retry_preferences_save();
    assert!(!fixture.app.preferences_not_saved());
    let written = std::fs::read_to_string(dir.join("writing_styles.toml")).unwrap();
    assert!(written.contains(id.as_str()), "{written}");
}

#[test]
fn a_drafted_reply_log_that_could_not_be_written_is_said_too() {
    let fixture = Fixture::stored(BuildKind::Development, |dir| {
        std::fs::create_dir(dir.join("drafted_replies.toml")).unwrap();
    });
    fixture
        .app
        .note_drafted_reply("acct-1", message("reply@example.eu"));
    fixture.app.settle_preferences();
    assert!(fixture.app.preferences_not_saved());
    assert!(
        fixture.app.is_drafted_reply(&message("reply@example.eu")),
        "held for this run whatever the disk said"
    );
}
