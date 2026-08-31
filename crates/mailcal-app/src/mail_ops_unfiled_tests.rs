//! The unfiled-Sent-copy question: how a delivered message with no copy in Sent surfaces,
//! and what retrying or dismissing it does.
//!
//! A child of [`super`] (the send tests), reusing its `SubmitProvider` and app builders;
//! split into its own file to keep each test module under the 500-line limit.

use std::sync::Arc;

use tokio::sync::Notify;

use super::{SubmitProvider, app_over, dispatch_until, plain_send};
use crate::{Intent, SendStatus};

/// A second plain send, distinguishable from [`plain_send`] by its subject.
fn other_send() -> Intent {
    Intent::SubmitMail {
        to: "you@test.local".to_owned(),
        subject: "Second".to_owned(),
        body: "Body".to_owned(),
    }
}

/// **The regression, at the layer the user sees.** A message that goes out but whose Sent
/// copy could not be filed reaches its own terminal state: not `Sent`, which is what let a
/// delivered-but-unfiled message read as a clean send all the way to the UI, and not
/// `Failed`, which would say a message that has already reached its recipients did not go.
///
/// And unlike the status, the **question** it raises does not expire: the hint is gone two
/// and a half seconds later, while the thing the user can act on stands until they act.
#[tokio::test(start_paused = true)]
async fn a_send_whose_copy_was_not_filed_reaches_its_own_terminal_state() {
    let app = app_over(SubmitProvider::filing_nothing());

    let task = dispatch_until(&app, plain_send(), SendStatus::SentNotFiled).await;

    assert_eq!(app.send_status(), SendStatus::SentNotFiled);
    let pending = app.unfiled_copy().expect("the question is standing");
    assert_eq!(pending.subject, "Hi");
    assert!(!pending.retrying);

    // The transient hint clears like any other terminal state; the question does not.
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();
    assert_eq!(app.send_status(), SendStatus::Idle);
    assert!(
        app.unfiled_copy().is_some(),
        "a missing Sent copy is not something that times out"
    );
}

/// Answering the question files the copy and clears it, and files it by **placing the
/// copy**, never by sending the message again. That distinction is the whole reason the
/// repair is a separate operation: re-running the send would put the message in front of its
/// recipients twice.
#[tokio::test(start_paused = true)]
async fn filing_the_copy_clears_the_question_without_re_sending() {
    let provider = SubmitProvider::filing_nothing();
    let (refiles, submissions) = (provider.refiles(), provider.submissions());
    let app = app_over(provider);
    let task = dispatch_until(&app, plain_send(), SendStatus::SentNotFiled).await;
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();
    assert_eq!(submissions.lock().unwrap().len(), 1);

    app.dispatch(Intent::RetryUnfiledCopy).await;

    assert!(app.unfiled_copy().is_none(), "the copy is filed");
    assert_eq!(*refiles.lock().unwrap(), 1);
    assert_eq!(
        submissions.lock().unwrap().len(),
        1,
        "the message was not sent a second time"
    );
}

/// A repair that fails leaves the question standing, carrying the new reason, so the button
/// can be pressed again. A modal that dismissed itself on failure would take the user's only
/// handle on the problem with it.
#[tokio::test(start_paused = true)]
async fn a_failed_repair_leaves_the_question_answerable() {
    let provider = SubmitProvider::filing_nothing_ever();
    let refiles = provider.refiles();
    let app = app_over(provider);
    let task = dispatch_until(&app, plain_send(), SendStatus::SentNotFiled).await;
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();

    app.dispatch(Intent::RetryUnfiledCopy).await;

    let still = app.unfiled_copy().expect("the question is still standing");
    assert!(!still.retrying, "and it is pressable again");
    assert!(still.detail.contains("still unreachable"));

    app.dispatch(Intent::RetryUnfiledCopy).await;
    assert_eq!(*refiles.lock().unwrap(), 2);
}

/// Dismissing accepts the missing copy: nothing is filed, and nothing is sent.
#[tokio::test(start_paused = true)]
async fn dismissing_the_question_files_nothing() {
    let provider = SubmitProvider::filing_nothing();
    let refiles = provider.refiles();
    let app = app_over(provider);
    let task = dispatch_until(&app, plain_send(), SendStatus::SentNotFiled).await;
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    task.await.unwrap();

    app.dispatch(Intent::DismissUnfiledCopy).await;

    assert!(app.unfiled_copy().is_none());
    assert_eq!(*refiles.lock().unwrap(), 0);
}

/// **A repair must never answer a question that is not the one it started from.**
///
/// Two sends can fail to file in a row, and the second one replaces the standing question
/// while the first one's repair is still out on the network. Writing that repair's result
/// back unconditionally is the dangerous shape: on success it would clear a question nobody
/// answered; losing the *second* message's copy for good, since nothing later rediscovers a
/// copy that was never written to the server, and on failure it would relabel the second
/// question with the first message's subject and reason.
#[tokio::test(start_paused = true)]
async fn a_repair_in_flight_does_not_answer_a_newer_question() {
    let gate = Arc::new(Notify::new());
    let provider = SubmitProvider::filing_nothing_until(&gate);
    let refiles = provider.refiles();
    let app = app_over(provider);

    // The first send fails to file, raising the question the user presses.
    let first = dispatch_until(&app, plain_send(), SendStatus::SentNotFiled).await;
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    first.await.unwrap();
    let repair = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(Intent::RetryUnfiledCopy).await }
    });
    // Park until the repair is actually inside `file_sent_copy` and holding the gate.
    while *refiles.lock().unwrap() == 0 {
        tokio::task::yield_now().await;
    }

    // A second send fails to file while that repair is still out, and takes the question over.
    let second = dispatch_until(&app, other_send(), SendStatus::SentNotFiled).await;
    tokio::time::advance(crate::mail_ops::AUTO_CLEAR_DELAY).await;
    second.await.unwrap();
    assert_eq!(
        app.unfiled_copy().map(|u| u.subject),
        Some("Second".to_owned()),
        "the newer send owns the question"
    );

    // Now let the first repair finish successfully. Its copy is filed, but the question on
    // screen belongs to the second message and must survive untouched.
    gate.notify_waiters();
    repair.await.unwrap();

    let standing = app
        .unfiled_copy()
        .expect("the second message's question is still standing");
    assert_eq!(standing.subject, "Second");
    assert!(!standing.retrying, "and it is still pressable");
}
