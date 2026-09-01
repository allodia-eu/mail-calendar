//! Tests for the **editable subject** on a reply and a forward. A child of [`super`] (the
//! reply/forward tests), reusing its `reply_app`/`original_message`/`reply_document`/
//! `dispatch_until` fixtures; split into its own file so each test module stays under the
//! 500-line limit.

use super::{dispatch_until, original_message, reply_app, reply_document};
use crate::{Intent, MessageRef, SendStatus};

#[tokio::test(start_paused = true)]
async fn an_edited_subject_replaces_the_derived_one_on_a_reply() {
    // The composer's Subject field is editable on a reply, so a user who renames the thread
    // sends under the name they chose, not under `Re: <the original>`.
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: Some("Budget, split off the quarterly thread".to_owned()),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    let submissions = submissions.lock().unwrap();
    assert_eq!(
        submissions[0].subject,
        "Budget, split off the quarterly thread"
    );
}

#[tokio::test(start_paused = true)]
async fn an_edited_subject_replaces_the_derived_one_on_a_forward() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichForward {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "dest@forward.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: Some("For your files".to_owned()),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions[0].subject, "For your files");
}

#[tokio::test(start_paused = true)]
async fn a_cleared_subject_is_honoured_rather_than_refilled() {
    // An empty field is a decision, not a missing value: only `None` (a caller with no subject
    // field at all, such as the MCP surface) falls back to the derived `Re:`. Refilling a
    // cleared field would put a subject on the wire the user had deliberately removed.
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: Some(String::new()),
        document,
        blobs,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    assert_eq!(submissions.lock().unwrap()[0].subject, "");
}
