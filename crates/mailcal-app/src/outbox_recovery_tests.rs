//! A send whose process ended mid-attempt, and a send nobody answered: what the Outbox shows
//! afterwards, and what may send it again.
//!
//! The line is the hand-over (`docs/sending.md`). A send stopped before it is retried as if it
//! had never started, because nothing of it can have reached the server. A send stopped after it
//! may already be with its recipients, so nothing sends it again until the user says it did not
//! arrive. Every test counts hand-overs, because each one is a message that may have been
//! delivered: a count of two where one is expected is a message the recipients got twice.
//!
//! A child of the send tests, reusing their `SubmitProvider` and app builders. The process
//! ending is the attempt's task being dropped mid-await, which is what an exit does to it, and
//! the restart is [`App::recover_outbox`], which the boot path runs before anything can drain.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use mailcal_viewmodel::{QueuedRow, QueuedState};

use super::{
    SubmitProvider, app_over, dispatch_until, fake::CutOff, outbox_tests::outbox_holding,
    plain_send,
};
use crate::{App, Intent, OutboxIntent, QueuedRef, SendStatus};

/// Starts a send and ends it where the provider stops, as quitting the app there would.
async fn send_and_quit(app: &Arc<App<SubmitProvider>>, stopped: &AtomicUsize) {
    let task = tokio::spawn({
        let app = Arc::clone(app);
        async move { app.dispatch(plain_send()).await }
    });
    for _ in 0..100_000 {
        if stopped.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(
        stopped.load(Ordering::SeqCst),
        1,
        "the send never reached its stop"
    );
    task.abort();
    let _ = task.await;
}

/// What the Outbox shows once the app has started again.
async fn restart(app: &Arc<App<SubmitProvider>>) -> QueuedRow {
    app.recover_outbox().await;
    app.dispatch(Intent::Outbox(OutboxIntent::Show)).await;
    outbox_holding(app, 1).await.remove(0)
}

fn target(row: &QueuedRow) -> QueuedRef {
    QueuedRef::from_parts(&row.account, row.op).unwrap()
}

/// Going offline and back, which hurries everything that is waiting.
async fn reconnect(app: &Arc<App<SubmitProvider>>) {
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(Intent::ReportNetworkReachable(true)).await;
}

/// **The question this whole file answers.** Send pressed, the app closed while it was still
/// connecting: the message is in the Outbox at the next start, waiting rather than stuck as
/// "Sending", and goes out once.
#[tokio::test(start_paused = true)]
async fn a_send_cut_off_while_connecting_goes_out_once_after_a_restart() {
    let provider = SubmitProvider::cut_off_at(CutOff::BeforeHandOver);
    let (stopped, handed) = (provider.stopped(), provider.handed_over());
    let app = app_over(provider);
    send_and_quit(&app, &stopped).await;

    let row = restart(&app).await;
    assert_eq!(
        row.state,
        QueuedState::Waiting,
        "nothing reached the server"
    );
    assert_eq!(row.subject, "Hi");

    reconnect(&app).await;
    assert!(app.mailbox_list().outbox.is_empty(), "it went out");
    assert_eq!(handed.load(Ordering::SeqCst), 1, "and exactly once");
}

/// Cut off after the hand-over, it may already be with its recipients. It waits for the user,
/// and nothing that would send a waiting message sends this one.
#[tokio::test(start_paused = true)]
async fn a_send_cut_off_after_the_hand_over_is_never_sent_again_unasked() {
    let provider = SubmitProvider::cut_off_at(CutOff::AfterHandOver);
    let (stopped, handed) = (provider.stopped(), provider.handed_over());
    let app = app_over(provider);
    send_and_quit(&app, &stopped).await;

    let row = restart(&app).await;
    assert_eq!(row.state, QueuedState::Unconfirmed);

    reconnect(&app).await;
    for intent in [
        OutboxIntent::SendNow(target(&row)),
        OutboxIntent::Cancel(target(&row)),
        OutboxIntent::Edit(target(&row)),
    ] {
        app.dispatch(Intent::Outbox(intent)).await;
    }
    assert_eq!(
        handed.load(Ordering::SeqCst),
        1,
        "it was never attempted again"
    );
    assert!(app.compose_request().is_none(), "nor offered back to edit");
    let still = outbox_holding(&app, 1).await;
    assert_eq!(
        still[0].state,
        QueuedState::Unconfirmed,
        "and it is still there"
    );
}

/// The user says it did not arrive: it is sent now, once.
#[tokio::test(start_paused = true)]
async fn a_send_the_user_says_did_not_arrive_goes_out_again() {
    let provider = SubmitProvider::cut_off_at(CutOff::AfterHandOver);
    let (stopped, handed) = (provider.stopped(), provider.handed_over());
    let app = app_over(provider);
    send_and_quit(&app, &stopped).await;
    let row = restart(&app).await;

    app.dispatch(Intent::Outbox(OutboxIntent::ConfirmNotSent(target(&row))))
        .await;

    assert!(app.mailbox_list().outbox.is_empty());
    assert_eq!(handed.load(Ordering::SeqCst), 2);
}

/// The user says it arrived: it leaves the Outbox and nothing sends it again.
#[tokio::test(start_paused = true)]
async fn a_send_the_user_says_arrived_leaves_the_outbox_unsent() {
    let provider = SubmitProvider::cut_off_at(CutOff::AfterHandOver);
    let (stopped, handed) = (provider.stopped(), provider.handed_over());
    let app = app_over(provider);
    send_and_quit(&app, &stopped).await;
    let row = restart(&app).await;

    app.dispatch(Intent::Outbox(OutboxIntent::ConfirmSent(target(&row))))
        .await;
    reconnect(&app).await;

    assert!(app.mailbox_list().outbox.is_empty());
    assert_eq!(handed.load(Ordering::SeqCst), 1);
}

/// A send handed over with no answer back is reported as such, not as waiting: the hint must
/// not promise it will go by itself, and nothing sends it again.
#[tokio::test(start_paused = true)]
async fn an_unanswered_send_awaits_confirmation_and_is_not_resent() {
    let provider = SubmitProvider::unanswered_for(1);
    let handed = provider.handed_over();
    let app = app_over(provider);

    let task = dispatch_until(&app, plain_send(), SendStatus::Unconfirmed).await;
    assert_eq!(app.send_status(), SendStatus::Unconfirmed);
    let row = outbox_holding(&app, 1).await.remove(0);
    assert_eq!(row.state, QueuedState::Unconfirmed);
    task.await.unwrap();

    reconnect(&app).await;
    assert_eq!(handed.load(Ordering::SeqCst), 1);
}

/// An answer is only taken from a send that asked for one. A waiting message the user marks
/// as sent is not dropped, and one they say did not arrive is not sent an extra time: the
/// row may have changed state under the click.
#[tokio::test(start_paused = true)]
async fn an_answer_to_a_send_that_asked_nothing_changes_nothing() {
    let provider = SubmitProvider::offline_for(1);
    let submissions = provider.submissions();
    let app = app_over(provider);
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    let row = outbox_holding(&app, 1).await.remove(0);

    app.dispatch(Intent::Outbox(OutboxIntent::ConfirmSent(target(&row))))
        .await;
    app.dispatch(Intent::Outbox(OutboxIntent::ConfirmNotSent(target(&row))))
        .await;

    let still = outbox_holding(&app, 1).await;
    assert_eq!(still[0].state, QueuedState::Waiting);
    assert_eq!(submissions.lock().unwrap().len(), 1, "no attempt was added");
}

/// The log is the only record of what happened to a send nobody watched go out, so each step of
/// this one leaves a line: the attempt starting, the restart finding it, the queue at start-up,
/// and the retry going through. None of them carries the message.
#[tokio::test(start_paused = true)]
async fn a_recovered_send_leaves_a_line_at_every_step_and_none_of_the_message() {
    crate::tests_log_capture::install();
    let provider = SubmitProvider::cut_off_at(CutOff::BeforeHandOver);
    let stopped = provider.stopped();
    let app = app_over(provider);
    send_and_quit(&app, &stopped).await;
    restart(&app).await;
    reconnect(&app).await;

    let lines = crate::tests_log_capture::lines_from(&[
        "mailcal_app::mail_ops::send",
        "mailcal_app::outbox_ops::outbox_log",
        "mailcal_app::outbox_ops",
    ]);
    for needle in [
        "send[a0]: submitting a message",
        "were cut off when the app last stopped",
        "outbox[a0]: at start-up, 1 send(s) waiting",
        "outbox[a0]: retrying queued send",
        "went through",
    ] {
        assert!(
            lines.iter().any(|line| line.contains(needle)),
            "no line contains {needle:?}; captured:\n{}",
            lines.join("\n")
        );
    }
    for private in ["you@test.local", "me@allodia.local", "Body"] {
        assert!(
            lines.iter().all(|line| !line.contains(private)),
            "a log line carries {private:?}"
        );
    }
}
