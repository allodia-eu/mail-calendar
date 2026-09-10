//! Tests for **rich forward**: the threading that keeps a forward on the conversation it came
//! from, and the original's files travelling with it. A child of [`super`] (the reply tests),
//! reusing its `reply_app`/`original_message`/`reply_document`/`dispatch_until` fixtures; split
//! into its own file so each test module stays under the 500-line limit.

use engine_api::MessageIdHeader;

use super::{
    ThreadProvider, app_over, dispatch_until, original_message, reply_app, reply_document,
};
use crate::{Intent, MessageRef, SendStatus};

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

/// Forwarding a message sends the message on, files included. The original's `invoice.pdf`
/// goes out beside whatever the user wrote and attached in the composer, keeping the name and
/// media type the sender gave it, and the quoted body's inline logo is **not** duplicated as a
/// file: it is re-attached as the `cid:` part the quote references.
#[tokio::test(start_paused = true)]
async fn rich_forward_carries_the_originals_attachments() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "dest@forward.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    let draft = &submissions[0];
    // The composer's own two parts (an inline chart and a file the user attached) plus the
    // one file the original carried.
    assert_eq!(draft.attachments.len(), 3);
    let forwarded = draft
        .attachments
        .iter()
        .find(|attachment| attachment.file_name == "invoice.pdf")
        .expect("the original's file must travel with the forward");
    assert_eq!(forwarded.media_type, "application/pdf");
    assert_eq!(forwarded.content, b"INVOICE");
    assert!(
        !forwarded.is_inline(),
        "a forwarded file is a file, not a body part"
    );
    // The original's inline logo is a `cid:` part of the quote, never a second file.
    assert!(
        !draft
            .attachments
            .iter()
            .any(|attachment| attachment.content == b"hello" && !attachment.is_inline()),
        "the original's inline image must not also go out as an attachment"
    );
}

/// A reply is not a forward: it answers the message rather than passing it on, so the
/// original's files stay where they are. Sending them back to the person who sent them is
/// noise, and on a long thread it is the same file again on every turn.
#[tokio::test(start_paused = true)]
async fn rich_reply_leaves_the_originals_attachments_behind() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    let draft = &submissions[0];
    assert_eq!(draft.attachments.len(), 2, "the composer's two parts only");
    assert!(
        !draft
            .attachments
            .iter()
            .any(|attachment| attachment.file_name == "invoice.pdf"),
        "a reply does not send the original's files back"
    );
}

/// A forward whose files cannot be read does not go out. The alternative is a message that
/// says "see attached" with nothing attached, which neither the sender nor the recipient can
/// tell is incomplete, and a send cannot be taken back. A failed send can be tried again.
#[tokio::test(start_paused = true)]
async fn rich_forward_that_cannot_read_the_originals_files_does_not_send() {
    // Syncing needs no raw source, so the original is in the store and only reading its
    // parts fails: the account offline, or the message gone from the server.
    let (app, submissions) =
        app_over(ThreadProvider::with(vec![original_message("m1")]).failing_source());
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "dest@forward.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Failed).await;
    assert_eq!(app.send_status(), SendStatus::Failed);
    assert!(
        submissions.lock().unwrap().is_empty(),
        "nothing may go out without the files it was forwarding"
    );
}
