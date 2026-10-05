//! The Outbox, at the layer a client renders: a send that could not go out is **kept**, shown,
//! and acted on.
//!
//! The regression these exist for is the one that made the whole feature necessary: a failed
//! send reported `Failed`, the hint auto-cleared 2.5 seconds later, and the message was gone.
//! Nothing held a copy of it, because nothing could read the outbox back.
//!
//! A child of [`super`] (the send tests), reusing its `SubmitProvider` and app builders; split
//! into its own file to keep each test module under the 500-line limit.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use mailcal_viewmodel::{QueuedRow, QueuedState};

use super::{SubmitProvider, app_over, dispatch_until, plain_send};
use crate::{App, Intent, OutboxIntent, QueuedRef, SendStatus};

/// Waits until the Outbox holds `count` message(s), and answers with them.
///
/// The send status is set **before** the rebuild that reads the queue back, so reaching
/// `SendStatus::Queued` says nothing about the snapshot yet: a test that read `mailbox_list`
/// on that signal alone is reading whatever the last rebuild left. The rebuild's store reads
/// go through `spawn_blocking`, whose pool every test in this binary shares, so a fixed number
/// of yields is a race that opens under load and nowhere else. Waiting for the condition makes
/// the wait independent of how busy the machine is; the bound is what still turns a queue that
/// never fills into a failure rather than a hang.
pub(super) async fn outbox_holding(app: &Arc<App<SubmitProvider>>, count: usize) -> Vec<QueuedRow> {
    for _ in 0..100_000 {
        let outbox = app.mailbox_list().outbox;
        if outbox.len() == count {
            return outbox;
        }
        tokio::task::yield_now().await;
    }
    panic!(
        "the Outbox never reached {count} message(s): a send that did not go out must still be somewhere"
    )
}

/// **The regression.** A send that fails for a reason worth retrying is not lost: it reaches
/// its own status, and the message is in the Outbox with its recipients and subject intact.
///
/// The **server** is what is not answering here; the device is online, so the write follow-up
/// syncs and republishes on its way past. The case where the device itself is offline is the
/// one below, and it is a different code path.
#[tokio::test(start_paused = true)]
async fn a_send_the_server_refused_is_kept_in_the_outbox() {
    let app = app_over(SubmitProvider::offline());

    let task = dispatch_until(&app, plain_send(), SendStatus::Queued).await;
    assert_eq!(app.send_status(), SendStatus::Queued);

    let outbox = outbox_holding(&app, 1).await;
    let queued = &outbox[0];
    assert_eq!(queued.subject, "Hi");
    assert_eq!(queued.to, "you@test.local");
    assert_eq!(queued.account, "acct-1");
    assert_eq!(queued.attempts, 1);
    task.await.unwrap();
}

/// Waits until a sync is parked on the silent server, so what a test reads next is what the
/// user sees while that sync has still not come back.
async fn until_a_sync_is_parked(parked: &AtomicUsize) {
    for _ in 0..100_000 {
        if parked.load(Ordering::SeqCst) > 0 {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("no sync ever reached the silent server");
}

/// **A server that accepts the connection and then says nothing.** The send fails at its own
/// deadline and queues, but the sync that follows it has no deadline of its own, so an Outbox
/// published only by that sync's rebuild is not published for as long as the server stays
/// silent: the hint says "waiting to send" and the pane offers nowhere to look.
#[tokio::test(start_paused = true)]
async fn a_queued_send_reaches_the_outbox_while_the_server_stays_silent() {
    let provider = SubmitProvider::offline();
    provider.silence().store(true, Ordering::SeqCst);
    let parked = provider.parked_syncs();
    let app = app_over(provider);

    let task = dispatch_until(&app, plain_send(), SendStatus::Queued).await;
    until_a_sync_is_parked(&parked).await;

    let outbox = app.mailbox_list().outbox;
    assert_eq!(
        outbox.len(),
        1,
        "the queue is in the store, so the Outbox must not wait for the server to answer"
    );
    assert_eq!(outbox[0].subject, "Hi");
    task.abort();
}

/// The same for a queued send that goes out on "Send now": it leaves the Outbox when it is
/// delivered, not when the sync after it comes back.
#[tokio::test(start_paused = true)]
async fn a_delivered_send_leaves_the_outbox_while_the_server_stays_silent() {
    let provider = SubmitProvider::offline_for(1);
    let silence = provider.silence();
    let parked = provider.parked_syncs();
    let app = app_over(provider);
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    let queued = outbox_holding(&app, 1).await[0].clone();

    silence.store(true, Ordering::SeqCst);
    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.dispatch(Intent::Outbox(OutboxIntent::SendNow(
                QueuedRef::from_parts(&queued.account, queued.op).unwrap(),
            )))
            .await;
        }
    });
    until_a_sync_is_parked(&parked).await;

    assert!(
        app.mailbox_list().outbox.is_empty(),
        "the send went out, so it leaves the Outbox before the server is heard from again"
    );
    task.abort();
}

/// **The airplane-mode case, and the one a user actually meets.** A device with no network is
/// the ordinary reason a send queues, and it was the one case where the Outbox stayed empty:
/// the write follow-up has no server to re-read, returned without republishing the list, and
/// nothing else was going to rebuild it while the network was down. So the send hint said
/// "waiting to send" and the pane offered nowhere to look, until reconnecting fixed both at
/// once and hid the whole thing.
///
/// The queue is in the store, not on a server, so it is knowable offline and must be shown.
#[tokio::test(start_paused = true)]
async fn a_send_queued_with_the_device_offline_still_reaches_the_outbox() {
    let app = app_over(SubmitProvider::offline());
    app.dispatch(Intent::ReportNetworkReachable(false)).await;

    let task = dispatch_until(&app, plain_send(), SendStatus::Queued).await;
    assert_eq!(app.send_status(), SendStatus::Queued);

    let outbox = outbox_holding(&app, 1).await;
    assert_eq!(outbox[0].subject, "Hi");
    assert_eq!(outbox[0].to, "you@test.local");
    task.await.unwrap();
}

/// A send the server refused is **kept**: the user is told it failed, and the message waits in
/// the Outbox, never sent again on its own, until they send it again or discard it. Retrying it
/// unasked sends the same message to the same server for the same answer; dropping it loses
/// the user's words.
#[tokio::test(start_paused = true)]
async fn a_refused_send_stays_in_the_outbox_until_the_user_acts() {
    let provider = SubmitProvider::failing_with("550 mailbox unavailable");
    let submissions = provider.submissions();
    let app = app_over(provider);

    let task = dispatch_until(&app, plain_send(), SendStatus::Failed).await;
    assert_eq!(app.send_status(), SendStatus::Failed);
    let queued = outbox_holding(&app, 1).await[0].clone();
    assert_eq!(queued.state, QueuedState::NotSent);
    assert_eq!(queued.subject, "Hi");
    task.await.unwrap();

    // Reconnecting hurries what is waiting, never what the server refused.
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(Intent::ReportNetworkReachable(true)).await;
    assert_eq!(
        submissions.lock().unwrap().len(),
        1,
        "a refusal is never retried unasked"
    );

    let target = QueuedRef::from_parts(&queued.account, queued.op).unwrap();
    app.dispatch(Intent::Outbox(OutboxIntent::SendNow(target.clone())))
        .await;
    assert_eq!(
        submissions.lock().unwrap().len(),
        2,
        "sending it again is an attempt"
    );
    let again = outbox_holding(&app, 1).await;
    assert_eq!(again[0].state, QueuedState::NotSent);
    assert_eq!(again[0].attempts, 2);

    app.dispatch(Intent::Outbox(OutboxIntent::Cancel(target)))
        .await;
    assert!(
        app.mailbox_list().outbox.is_empty(),
        "discarding it is the one way it leaves"
    );
}

/// The Outbox is its own scope: selecting it shows the queued sends, not everyone's inbox.
///
/// `Scope::Outbox` names no account and no folder, exactly as the unified inbox does, so a
/// snapshot that could not tell them apart would answer the click with the wrong list.
#[tokio::test(start_paused = true)]
async fn showing_the_outbox_shows_queued_sends_rather_than_mail() {
    let app = app_over(SubmitProvider::offline());
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();

    outbox_holding(&app, 1).await;

    app.dispatch(Intent::Outbox(OutboxIntent::Show)).await;

    let snapshot = app.mailbox_list();
    assert!(snapshot.showing_outbox);
    assert!(snapshot.rows.is_empty(), "the Outbox holds no stored mail");
    assert_eq!(snapshot.outbox.len(), 1);
}

/// Withdrawing a queued send takes it out of the Outbox for good.
#[tokio::test(start_paused = true)]
async fn a_queued_send_can_be_withdrawn() {
    let app = app_over(SubmitProvider::offline());
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    let queued = outbox_holding(&app, 1).await[0].clone();

    app.dispatch(Intent::Outbox(OutboxIntent::Cancel(
        QueuedRef::from_parts(&queued.account, queued.op).unwrap(),
    )))
    .await;

    assert!(app.mailbox_list().outbox.is_empty());
}

/// "Send now" clears the backoff and drains in the same gesture, so the message goes out
/// while the user is still looking at it rather than at some later pass.
#[tokio::test(start_paused = true)]
async fn a_queued_send_goes_out_when_the_user_asks() {
    // One refusal, then the network is back.
    let app = app_over(SubmitProvider::offline_for(1));
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    let queued = outbox_holding(&app, 1).await[0].clone();

    app.dispatch(Intent::Outbox(OutboxIntent::SendNow(
        QueuedRef::from_parts(&queued.account, queued.op).unwrap(),
    )))
    .await;

    assert!(
        app.mailbox_list().outbox.is_empty(),
        "the send went out, so it leaves the Outbox"
    );
}

/// Editing a queued send **withdraws it first**, then hands the message to the host's
/// composer. The other order leaves a window in which a drain delivers the message the user
/// is editing, and there is no taking that back.
#[tokio::test(start_paused = true)]
async fn editing_a_queued_send_withdraws_it_before_offering_the_composer() {
    let app = app_over(SubmitProvider::offline());
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    let queued = outbox_holding(&app, 1).await[0].clone();

    app.dispatch(Intent::Outbox(OutboxIntent::Edit(
        QueuedRef::from_parts(&queued.account, queued.op).unwrap(),
    )))
    .await;

    assert!(
        app.mailbox_list().outbox.is_empty(),
        "it must leave the queue before the composer can hold it"
    );
    let request = app.compose_request().expect("the message is offered back");
    assert_eq!(request.subject, "Hi");
    assert_eq!(request.to, "you@test.local");
    assert_eq!(request.account, "acct-1");

    // Standing until the host says its composer has it: the Outbox no longer holds the
    // message, so an unanswered request is the only copy.
    app.dispatch(Intent::DismissComposeRequest).await;
    assert!(app.compose_request().is_none());
}

/// Coming back online sends what was written while there was no network, without the user
/// asking. This is the engine's wake signal: it runs no timer of its own.
#[tokio::test(start_paused = true)]
async fn reconnecting_sends_what_was_written_offline() {
    let app = app_over(SubmitProvider::offline_for(1));
    dispatch_until(&app, plain_send(), SendStatus::Queued)
        .await
        .await
        .unwrap();
    outbox_holding(&app, 1).await;

    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(Intent::ReportNetworkReachable(true)).await;

    assert!(
        app.mailbox_list().outbox.is_empty(),
        "coming back online must flush the queue"
    );
}
