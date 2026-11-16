//! A composition's files across the FFI, through every way a composer opens and ends
//! (`docs/composer-layout.md`, rules 3 and 8).

use std::sync::{Arc, mpsc};

use crate::{
    ComposeOrigin, ComposerFileAttachment, ComposerState, LogLevel, MailcalApp, MovedSignature,
    Recipients,
    tests::{ChannelObserver, NullLogger},
};

const DOCUMENT: &str = r#"{"blocks":[{"Paragraph":{"content":[{"Text":{"text":"Hello","bold":false,"italic":false,"underline":false}}]}}],"attachments":[]}"#;

fn app() -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    )
}

fn staging() -> String {
    std::env::temp_dir()
        .join(format!("mailcal-composer-files-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn open_new(app: &MailcalApp, composition: &str) {
    app.open_composer(
        composition.to_owned(),
        ComposeOrigin::New { account: None },
        staging(),
        None,
    )
    .expect("the composer opens");
}

fn picked(name: &str) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: format!("/picked/{name}"),
        file_name: name.to_owned(),
        media_type: String::new(),
    }
}

fn names(app: &MailcalApp, composition: &str) -> Vec<String> {
    app.composer_files(composition.to_owned())
        .expect("files")
        .tiles
        .into_iter()
        .map(|tile| tile.file_name)
        .collect()
}

#[test]
fn a_new_composer_opens_holding_nothing_and_a_picked_file_is_a_change() {
    let app = app();
    open_new(&app, "c1");
    let opened = app.composer_files("c1".to_owned()).expect("files");
    assert!(opened.tiles.is_empty() && !opened.changed);
    let added = app
        .add_composer_files("c1".to_owned(), vec![picked("a.pdf")])
        .expect("added");
    assert!(added.changed);
    assert_eq!(names(&app, "c1"), ["a.pdf"]);
}

#[test]
fn a_move_keeps_the_files_and_a_close_lets_them_go() {
    let app = app();
    open_new(&app, "c1");
    app.add_composer_files("c1".to_owned(), vec![picked("a.pdf")])
        .expect("added");
    let state = ComposerState {
        from: None,
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Moving".to_owned(),
        document_json: DOCUMENT.to_owned(),
        signature: MovedSignature::FollowsAccount,
        written: true,
    };
    app.open_composer(
        "c1".to_owned(),
        ComposeOrigin::Moved { state },
        staging(),
        None,
    )
    .expect("the composer moves");
    assert_eq!(
        names(&app, "c1"),
        ["a.pdf"],
        "the window holds what the pane held"
    );
    assert!(app.composer_files("c1".to_owned()).expect("files").changed);

    app.close_composition("c1".to_owned()).expect("closed");
    assert!(names(&app, "c1").is_empty());
}

#[test]
fn reopening_a_composition_starts_its_list_again() {
    let app = app();
    open_new(&app, "c1");
    app.add_composer_files("c1".to_owned(), vec![picked("a.pdf")])
        .expect("added");
    open_new(&app, "c1");
    assert!(names(&app, "c1").is_empty());
}

#[test]
fn an_accepted_send_and_a_discard_let_the_files_go() {
    let app = app();
    open_new(&app, "sent");
    open_new(&app, "thrown");
    app.submit_composition(
        "sent".to_owned(),
        Recipients {
            to: "someone@test.local".to_owned(),
            cc: String::new(),
            bcc: String::new(),
        },
        "Hello".to_owned(),
        DOCUMENT.to_owned(),
        None,
    )
    .expect("the send is accepted");
    assert!(names(&app, "sent").is_empty());

    app.add_composer_files("thrown".to_owned(), vec![picked("b.pdf")])
        .expect("added");
    app.discard_draft("thrown".to_owned()).expect("discarded");
    assert!(names(&app, "thrown").is_empty());
}

#[test]
fn a_send_whose_file_cannot_be_read_is_refused_and_keeps_the_list() {
    let app = app();
    open_new(&app, "c1");
    app.add_composer_files("c1".to_owned(), vec![picked("gone.pdf")])
        .expect("added");
    let refused = app.submit_composition(
        "c1".to_owned(),
        Recipients {
            to: "someone@test.local".to_owned(),
            cc: String::new(),
            bcc: String::new(),
        },
        "Hello".to_owned(),
        DOCUMENT.to_owned(),
        None,
    );
    assert!(refused.is_err());
    assert_eq!(
        names(&app, "c1"),
        ["gone.pdf"],
        "the composer stays as it was"
    );
}
