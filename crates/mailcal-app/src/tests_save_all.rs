//! Saving every attachment of the open message into a folder at once
//! (`docs/reading-actions.md`, "Saving every attachment").

use std::{
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use fakes::{FakeProvider, account, app, message, msg};

use super::offers_save_all;
use crate::{Intent, ReaderId, tests_fakes as fakes};

/// A message carrying `files`, each a `(file name, base64 body)` pair.
fn message_with(files: &[(&str, &str)]) -> String {
    let mut raw = String::from(concat!(
        "Content-Type: multipart/mixed; boundary=\"m\"\r\n\r\n",
        "--m\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody\r\n",
    ));
    for (name, body) in files {
        let _ = write!(
            raw,
            "--m\r\nContent-Type: application/pdf\r\n\
             Content-Disposition: attachment; filename=\"{name}\"\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n{body}\r\n"
        );
    }
    raw.push_str("--m--\r\n");
    raw
}

async fn opened(raw: &str) -> crate::App<FakeProvider> {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider =
        FakeProvider::with(vec![message("m1", "a", "Quotes")]).with_source(raw.as_bytes());
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: msg("acct-1", "m1"),
    })
    .await;
    app
}

fn names(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[tokio::test]
async fn every_attachment_lands_in_the_folder_under_its_own_name() {
    let app = opened(&message_with(&[("a.pdf", "QUFB"), ("b.pdf", "QkJC")])).await;
    let folder = tempfile::tempdir().expect("a folder");

    let saved = app
        .save_all_attachments(msg("acct-1", "m1"), folder.path().to_str().expect("path"))
        .await
        .expect("saved");

    assert_eq!(names(&saved), ["a.pdf", "b.pdf"]);
    assert_eq!(
        std::fs::read(folder.path().join("a.pdf")).expect("a"),
        b"AAA"
    );
    assert_eq!(
        std::fs::read(folder.path().join("b.pdf")).expect("b"),
        b"BBB"
    );
}

#[tokio::test]
async fn nothing_in_the_folder_is_written_over() {
    // Two files of one name are routine (a scanner's `scan.pdf`, twice), and the folder the
    // reader picked may already hold one: every file is kept, the later ones numbered.
    let app = opened(&message_with(&[("scan.pdf", "QUFB"), ("scan.pdf", "QkJC")])).await;
    let folder = tempfile::tempdir().expect("a folder");
    std::fs::write(folder.path().join("scan.pdf"), b"mine").expect("an existing file");

    let saved = app
        .save_all_attachments(msg("acct-1", "m1"), folder.path().to_str().expect("path"))
        .await
        .expect("saved");

    assert_eq!(names(&saved), ["scan (2).pdf", "scan (3).pdf"]);
    assert_eq!(
        std::fs::read(folder.path().join("scan.pdf")).expect("kept"),
        b"mine"
    );
    assert_eq!(
        std::fs::read(folder.path().join("scan (2).pdf")).expect("2"),
        b"AAA"
    );
    assert_eq!(
        std::fs::read(folder.path().join("scan (3).pdf")).expect("3"),
        b"BBB"
    );
}

#[tokio::test]
async fn a_name_no_filesystem_accepts_is_made_one() {
    // The sender's name is free text: Windows refuses `:` and `?`, and a right-to-left
    // override makes `exe` read as `pdf`.
    let app = opened(&message_with(&[("Q3: totals?\u{202E}fdp.exe", "QUFB")])).await;
    let folder = tempfile::tempdir().expect("a folder");

    let saved = app
        .save_all_attachments(msg("acct-1", "m1"), folder.path().to_str().expect("path"))
        .await
        .expect("saved");

    let [name] = names(&saved).try_into().expect("one file");
    assert!(name.starts_with("Q3_ totals_"), "{name}");
    assert!(name.ends_with("fdp.exe"), "{name}");
    assert!(!name.contains(['\u{202E}', ':', '?']), "{name}");
}

#[tokio::test]
async fn a_folder_that_cannot_be_written_is_an_error_and_leaves_nothing() {
    let app = opened(&message_with(&[("a.pdf", "QUFB"), ("b.pdf", "QkJC")])).await;
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let not_a_folder = scratch.path().join("file");
    std::fs::write(&not_a_folder, b"").expect("a file");

    let result = app
        .save_all_attachments(msg("acct-1", "m1"), not_a_folder.to_str().expect("path"))
        .await;

    assert!(result.is_err(), "{result:?}");
    let left: Vec<_> = std::fs::read_dir(scratch.path())
        .expect("listed")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(left, ["file"]);
}

#[test]
fn save_all_is_offered_once_there_is_more_than_one_file() {
    // With one file, Save beside it already does exactly this.
    assert!(!offers_save_all(0));
    assert!(!offers_save_all(1));
    assert!(offers_save_all(2));
    assert!(offers_save_all(20));
}
