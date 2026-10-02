//! What a send does to the draft the composer was writing (`docs/drafts.md`).
//!
//! An accepted send takes the stored copy away, a failed one leaves it, and either way the
//! composition is finished with: the composer was dismissed the moment the submit was accepted,
//! long before the message had been anywhere, so nothing is left that could still save under it.
//!
//! Every test here awaits the send **to completion**, not merely until the hint moves: the draft
//! is taken away after the status says the message went, so asserting on the hint alone would race
//! the removal, and the tests that assert nothing was removed would pass while it was still to
//! come.
//!
//! A child of [`super`], reusing its fixtures; its own file to keep each test module under the
//! 500-line limit.

use engine_api::ProviderKey;

use super::{
    Intent, SendStatus, ThreadProvider, app_and_logs, composition, dispatch_until, document,
    draft_ref, staging_dir, stored_draft,
};
use crate::DraftsIntent;

/// A send that was accepted takes the composition's stored draft away.
///
/// Otherwise every message composed over more than half a minute is filed twice: once in
/// Sent, where the user expects it, and once in Drafts, where they find it weeks later and
/// cannot tell whether it went.
#[tokio::test(start_paused = true)]
async fn an_accepted_send_takes_the_stored_draft_away() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("sent"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("Finished at last"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    dispatch_until(&app, intent, SendStatus::Sent)
        .await
        .await
        .expect("the send finishes");

    let deletes = logs.draft_deletes.lock().unwrap();
    assert_eq!(
        deletes.as_slice(),
        &[ProviderKey::new("d1").unwrap()],
        "the message has gone out, so the copy waiting in Drafts goes with it"
    );
}

/// A send that **failed** keeps the draft.
///
/// This is the one outcome where the stored copy is the only one left: the message never
/// reached a server, nothing will retry it, and the host has already dismissed the composer
/// that held the words. Removing it here would be the send losing the user's message.
#[tokio::test(start_paused = true)]
async fn a_failed_send_leaves_the_draft_where_the_words_are() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).failing_send());
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("failed"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("Never got there"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    dispatch_until(&app, intent, SendStatus::Failed)
        .await
        .await
        .expect("the send finishes");

    assert!(
        logs.draft_deletes.lock().unwrap().is_empty(),
        "nothing else holds the message, so the draft stays"
    );
}

/// A send from a composer that never saved reaches no server about a draft.
///
/// The ordinary case: most messages are written and sent inside the idle interval, so the
/// cleanup must be a no-op rather than a delete of whatever key happens to be around.
#[tokio::test(start_paused = true)]
async fn a_send_from_a_composer_that_never_saved_removes_nothing() {
    let (app, logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    app.dispatch(Intent::RefreshMail).await;

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Quick one".to_owned(),
        document: document("Sent straight away"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    dispatch_until(&app, intent, SendStatus::Sent)
        .await
        .await
        .expect("the send finishes");

    assert!(logs.draft_deletes.lock().unwrap().is_empty());
}

/// A send with no network takes the draft away too: the message is **in the outbox**, which
/// is durable, so the stored copy is already a duplicate of something on its way.
///
/// The rule is "accepted", not "delivered". Waiting for delivery instead would leave a
/// message sent from a train filed in Drafts for good, because nothing revisits the
/// composition once the drain settles it.
#[tokio::test(start_paused = true)]
async fn a_queued_send_takes_the_stored_draft_away_as_well() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).queueing_send());
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("queued"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("On the train"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    dispatch_until(&app, intent, SendStatus::Queued)
        .await
        .await
        .expect("the send finishes");

    let deletes = logs.draft_deletes.lock().unwrap();
    assert_eq!(deletes.as_slice(), &[ProviderKey::new("d1").unwrap()]);
}

/// A **failed** send keeps the draft and still forgets the composition.
///
/// The composer was dismissed the moment the submit was accepted, so by the time the send has
/// settled there is nobody left to save under that id. Leaving the record would keep it for the
/// life of the process, and hand it to whatever composer took the id next.
#[tokio::test(start_paused = true)]
async fn a_failed_send_forgets_the_composition_it_would_not_discard() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).failing_send());
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(
        composition("c1"),
        draft_ref("d1"),
        &staging_dir("failed-forget"),
    )
    .await
    .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("Finished at last"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    dispatch_until(&app, intent, SendStatus::Failed)
        .await
        .await
        .expect("the send finishes");
    assert!(
        logs.draft_deletes.lock().unwrap().is_empty(),
        "nothing will retry the message, so the stored copy is the only one left"
    );

    // Asked of the record rather than of a field: a discard on a composition the core still
    // held would name its key and take the draft, which is exactly what must no longer be
    // reachable once the send has finished with it.
    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("c1"),
    }))
    .await;
    assert!(
        logs.draft_deletes.lock().unwrap().is_empty(),
        "the send finished with the composition, so nothing can still be saving under it"
    );
}
