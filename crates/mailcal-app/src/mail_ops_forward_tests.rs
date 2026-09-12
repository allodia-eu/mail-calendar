//! Tests for **rich forward**: the threading that keeps a forward on the conversation it came
//! from, and the staging that lets its composer open holding the files the original carries. A
//! child of [`super`] (the reply tests), reusing its
//! `reply_app`/`original_message`/`reply_document`/`dispatch_until` fixtures; split into its own
//! file so each test module stays under the 500-line limit.

use engine_api::MessageIdHeader;

use super::{
    ThreadProvider, app_over, dispatch_until, original_message, reply_app, reply_document,
};
use crate::{Intent, MessageRef, SendStatus};

/// A directory of this test's own under the system temp dir, emptied first so a previous run
/// cannot make an assertion pass.
fn staging_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mailcal-forward-staging-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

/// A forward belongs to the conversation it came from. It carries the original's
/// `References` chain (but no `In-Reply-To`, because it answers nothing) and that chain is
/// what puts the Sent copy on the thread. Without it, every forward the user sends becomes a
/// separate one-message conversation sitting beside the discussion it is part of.
#[tokio::test(start_paused = true)]
async fn rich_forward_sets_fwd_subject_and_threads_on_references() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "dest@forward.test".to_owned(),
        cc: "watcher@forward.test".to_owned(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions.len(), 1);
    let draft = &submissions[0];
    // Recipients are the explicitly given addresses; Fwd: subject.
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].email, "dest@forward.test");
    assert_eq!(draft.cc.len(), 1);
    assert_eq!(draft.cc[0].email, "watcher@forward.test");
    assert_eq!(draft.subject, "Fwd: Quarterly report");
    // Rich body present (a forward through the rich path still renders the composer).
    assert_eq!(draft.text_body, "Hello [Chart]");
    assert!(draft.html_body.is_some());
    // Threading: the original's chain plus the original itself, and no reply pointer.
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
    assert!(
        draft.in_reply_to.is_none(),
        "a forward continues a thread; it does not answer a message"
    );
}

/// Opening a forward stages the files the original carries, so the composer can show them as
/// ordinary attachments. Each keeps the name and media type the sender gave it, whatever the
/// file is called on disk, and the bytes are the sender's.
///
/// The quoted body's inline logo is **not** among them: it is a `cid:` part the quote
/// references, re-attached as one on send, so staging it would put the same picture in the
/// message twice, once as a file nobody attached.
#[tokio::test]
async fn staging_writes_the_files_the_original_carries() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let directory = staging_dir("carries");
    let staged = app
        .stage_forwarded_attachments(
            MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
            &directory,
        )
        .await
        .expect("the original's files stage");

    assert_eq!(staged.len(), 1, "the inline logo is not a file: {staged:?}");
    let file = &staged[0];
    assert_eq!(file.file_name, "invoice.pdf");
    assert_eq!(file.media_type, "application/pdf");
    assert_eq!(
        std::fs::read(&file.path).expect("the staged file is on disk"),
        b"INVOICE",
        "the staged bytes are the sender's"
    );
    assert!(
        file.path.starts_with(&directory),
        "staged into the directory the host named: {}",
        file.path
    );
}

/// A reference that resolves to no message is an error rather than an empty list, so a client
/// cannot read "this message has nothing attached" out of a message it never found.
#[tokio::test]
async fn staging_a_message_that_is_not_there_is_an_error() {
    let (app, _submissions) = reply_app(Vec::new());
    app.dispatch(Intent::RefreshMail).await;

    let staged = app
        .stage_forwarded_attachments(
            MessageRef::from_parts("acct-1", "absent".to_owned()).unwrap(),
            &staging_dir("absent"),
        )
        .await;
    assert!(staged.is_err(), "a message that is not there is not staged");
}

/// Files that cannot be read are reported, never quietly left out. The composer still opens,
/// and what the client must not do is show an attachment-less forward as if the message had
/// nothing attached: the user would send it without ever knowing.
#[tokio::test]
async fn staging_reports_files_it_cannot_read() {
    // Syncing needs no raw source, so the original is in the store and only reading its parts
    // fails: the account offline, or the message gone from the server.
    let (app, _submissions) =
        app_over(ThreadProvider::with(vec![original_message("m1")]).failing_source());
    app.dispatch(Intent::RefreshMail).await;

    let staged = app
        .stage_forwarded_attachments(
            MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
            &staging_dir("unreadable"),
        )
        .await;
    assert!(
        staged.is_err(),
        "unreadable files are an error, not an empty list: {staged:?}"
    );
}
