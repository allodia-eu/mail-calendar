//! Tests for the **result-returning** mail actions (`mail_ops::result`): the agent adapter's
//! write path.
//!
//! Two things are under test that the interactive path deliberately does not have: a caller can
//! tell *why* nothing happened (a closed [`MailActionError`], not a silent no-op), and a burst of
//! writes does not turn into a burst of account-wide syncs. Both are the difference between an
//! assistant reporting the truth and an assistant reporting success it never achieved.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_provider::MailEdit;
use fakes::{FakeProvider, account, app, message, msg};

use crate::{MailActionError, SendActionError};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn an_action_on_an_unconfigured_account_says_so() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m1", "a", "Hello")]),
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(
        app.act_mark_read(&msg("nope", "m1"), true).await,
        Err(MailActionError::UnknownAccount),
        "an unknown account is distinguishable from an unknown message",
    );
}

#[tokio::test]
async fn an_action_on_an_unknown_key_says_so() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m1", "a", "Hello")]),
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(
        app.act_mark_read(&msg("acct-1", "not-a-key"), true).await,
        Err(MailActionError::UnknownMessage),
    );
}

#[tokio::test]
async fn archiving_without_an_archive_folder_says_so_rather_than_failing_silently() {
    // The default fake advertises only an Inbox. The interactive path collapses this into a
    // logged no-op: an assistant would report "archived" over a message that never moved.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m1", "a", "Hello")]),
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(
        app.act_archive(&msg("acct-1", "m1")).await,
        Err(MailActionError::NoTargetFolder),
    );
}

#[tokio::test]
async fn a_provider_that_refuses_the_edit_surfaces_as_rejected() {
    let provider = FakeProvider::with(vec![message("m1", "a", "Hello")]);
    let refuse = provider.failure_switch();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;

    // The message is synced; now the server starts refusing writes (a revoked scope, an outage).
    refuse.store(true, Ordering::SeqCst);
    assert_eq!(
        app.act_mark_read(&msg("acct-1", "m1"), true).await,
        Err(MailActionError::Rejected),
    );
}

#[tokio::test]
async fn an_agent_action_takes_the_same_door_as_the_user() {
    // The point of routing writes through the interactive handlers rather than a parallel
    // implementation: an agent's archive is one behaviour with a user's: the same edit, to the
    // same provider, with the same optimistic hide.
    let provider = FakeProvider::with_archive(vec![message("m1", "a", "Hello")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(app.act_archive(&msg("acct-1", "m1")).await, Ok(()));

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1, "exactly one edit reached the provider");
    assert!(
        matches!(
            &edits[0],
            MailEdit::MoveTo { destination, .. } if destination.key().as_str() == "archive"
        ),
        "and it is the same move-to-Archive the swipe produces: {:?}",
        edits[0],
    );
    assert!(
        !app.mailbox_list()
            .rows
            .iter()
            .any(|row| format!("{row:?}").contains("m1")),
        "the row left the user's own list, exactly as it does on a swipe",
    );
}

#[tokio::test]
async fn a_burst_of_agent_writes_costs_one_sync_not_one_per_message() {
    // The trap this closes: `refresh_mail` syncs EVERY account, and the interactive path runs it
    // per action. Fifty scripted archives would be fifty full syncs against the user's own
    // server: an agent-shaped denial of service they are paying for.
    let provider = FakeProvider::with_archive(vec![
        message("m1", "a", "One"),
        message("m2", "a", "Two"),
        message("m3", "a", "Three"),
        message("m4", "a", "Four"),
        message("m5", "a", "Five"),
    ]);
    let syncs = provider.syncs();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;
    let before = syncs.load(Ordering::SeqCst);

    for key in ["m1", "m2", "m3", "m4", "m5"] {
        assert_eq!(app.act_archive(&msg("acct-1", key)).await, Ok(()));
    }

    let after = syncs.load(Ordering::SeqCst) - before;
    assert_eq!(
        after, 1,
        "five archives inside the coalescing window drove one account-wide sync, not five",
    );

    // Every archive still *happened*; throttling the follow-up sync must never throttle the
    // action itself, or an assistant's archive would silently not occur.
    assert!(
        app.mailbox_list().rows.is_empty(),
        "all five rows left the list",
    );
}

#[tokio::test]
async fn a_send_with_no_recipients_is_refused_rather_than_sent_blank() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    assert_eq!(
        app.act_send_plain(
            None,
            &[],
            &[],
            &["  ".to_owned()],
            "Hi".to_owned(),
            String::new()
        )
        .await,
        Err(SendActionError::NoRecipients),
        "a whitespace-only recipient is no recipient",
    );
}

#[tokio::test]
async fn an_archive_re_syncs_its_own_account_and_leaves_the_others_alone() {
    // The follow-up to a write went to every configured account. Nothing on the others can have
    // changed because of it (the edit never left this one) so a five-account user paid five
    // servers a round trip for one swipe.
    let acting = FakeProvider::with_archive(vec![message("m1", "a", "One")]);
    let bystander = FakeProvider::with(vec![message("m2", "a", "Two")]);
    let acted = acting.syncs();
    let watched = bystander.syncs();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", acting), account("acct-2", bystander)],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshMail).await;
    let (acted_before, watched_before) =
        (acted.load(Ordering::SeqCst), watched.load(Ordering::SeqCst));

    assert_eq!(app.act_archive(&msg("acct-1", "m1")).await, Ok(()));

    assert_eq!(
        acted.load(Ordering::SeqCst) - acted_before,
        1,
        "the account whose mail moved re-syncs, so the server's view settles the optimistic hide",
    );
    assert_eq!(
        watched.load(Ordering::SeqCst) - watched_before,
        0,
        "an account the edit never reached is not asked",
    );
    assert!(
        !app.mailbox_list()
            .rows
            .iter()
            .any(|row| format!("{row:?}").contains("m1")),
        "the archived row still left the list",
    );
}
