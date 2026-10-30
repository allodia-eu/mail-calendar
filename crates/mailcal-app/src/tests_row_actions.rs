//! Tests for what an action on a conversation row reaches (`docs/row-actions.md`): the batch
//! write the selection bar and a conversation's menu both dispatch, and the menu
//! [`App::row_menu`](crate::App::row_menu) answers from the published snapshot.
//!
//! Every conversation here is account `acct-1`'s, whose owner is `me@acct-1.local`: a message
//! from that address is the owner's own, and one carrying the draft keyword is a draft.

use std::sync::{Arc, Mutex};

use engine_core::mail::{Keyword, Message, SystemKeyword};
use engine_provider::MailEdit;
use fakes::{FakeProvider, account, app, msg, thread_ref, threaded};
use mailcal_viewmodel::{MessageKind, RowAction, RowMenu, ViewMode};

use crate::{App, BulkAction, Intent, RowRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// One message on thread `t`, in `mailbox`, at minute `minute`, of `kind`; `marks` holds `u`
/// for unread and `f` for flagged, and anything without `u` is read.
fn mail(key: &str, mailbox: &str, minute: u8, kind: MessageKind, marks: &str) -> Message {
    let mut message = threaded(key, mailbox, "Test thread", "t");
    message.received_at = Some(format!("2026-10-30T09:{minute:02}:00Z").parse().unwrap());
    let from = match kind {
        MessageKind::Received => "bob@example.test",
        MessageKind::Sent | MessageKind::Draft => "me@acct-1.local",
    };
    message.envelope.from = vec![engine_api::EmailAddress::new(from)];
    let mut mark = |keyword| {
        message.keywords.insert(Keyword::system(keyword));
    };
    if !marks.contains('u') {
        mark(SystemKeyword::Seen);
    }
    if marks.contains('f') {
        mark(SystemKeyword::Flagged);
    }
    if kind == MessageKind::Draft {
        mark(SystemKeyword::Draft);
    }
    message
}

/// The app over one account holding `messages`, synced and showing conversations.
async fn conversation_app(
    messages: Vec<Message>,
) -> (App<FakeProvider>, Arc<Mutex<Vec<MailEdit>>>) {
    let provider = FakeProvider::with_sent_and_archive(messages);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    (app, edits)
}

fn conversation() -> Vec<RowRef> {
    vec![RowRef::Thread(thread_ref("acct-1", "t"))]
}

/// The keys the keyword edits touched, sorted.
fn edited(edits: &[MailEdit]) -> Vec<String> {
    let mut keys: Vec<String> = edits
        .iter()
        .map(|edit| match edit {
            MailEdit::SetKeywords { target, .. } => target.as_str().to_owned(),
            other => panic!("expected only keyword edits, got {other:?}"),
        })
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn marking_a_conversation_read_reaches_only_what_was_received() {
    let (app, edits) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, "u"),
        mail("s1", "sent", 10, MessageKind::Sent, "u"),
        mail("r2", "a", 20, MessageKind::Received, ""),
    ])
    .await;

    app.dispatch(Intent::ActOnSelection {
        rows: conversation(),
        action: BulkAction::MarkRead,
    })
    .await;

    assert_eq!(edited(&edits.lock().unwrap()), ["r1", "r2"]);
}

#[tokio::test]
async fn flagging_a_conversation_flags_its_newest_received_message() {
    let (app, edits) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("r2", "a", 20, MessageKind::Received, ""),
        mail("s1", "sent", 30, MessageKind::Sent, ""),
    ])
    .await;

    app.dispatch(Intent::ActOnSelection {
        rows: conversation(),
        action: BulkAction::Flag,
    })
    .await;

    assert_eq!(edited(&edits.lock().unwrap()), ["r2"]);
}

#[tokio::test]
async fn unflagging_a_conversation_clears_every_flag_on_it() {
    let (app, edits) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, "f"),
        mail("s1", "sent", 10, MessageKind::Sent, "f"),
        mail("r2", "a", 20, MessageKind::Received, ""),
    ])
    .await;

    app.dispatch(Intent::ActOnSelection {
        rows: conversation(),
        action: BulkAction::Unflag,
    })
    .await;

    assert_eq!(edited(&edits.lock().unwrap()), ["r1", "s1"]);
}

#[tokio::test]
async fn a_conversation_move_leaves_its_draft_and_its_sent_copy() {
    let (app, edits) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("s1", "sent", 10, MessageKind::Sent, ""),
        mail("d1", "a", 20, MessageKind::Draft, ""),
    ])
    .await;

    app.dispatch(Intent::ActOnSelection {
        rows: conversation(),
        action: BulkAction::Archive,
    })
    .await;

    let moved: Vec<String> = edits
        .lock()
        .unwrap()
        .iter()
        .map(|edit| match edit {
            MailEdit::MoveTo { target, .. } => target.as_str().to_owned(),
            other => panic!("expected only moves, got {other:?}"),
        })
        .collect();
    assert_eq!(
        moved,
        ["r1"],
        "unfinished writing is never archived with the thread"
    );
}

#[tokio::test]
async fn a_draft_named_by_itself_still_moves() {
    let (app, edits) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("d1", "a", 20, MessageKind::Draft, ""),
    ])
    .await;

    app.dispatch(Intent::ActOnSelection {
        rows: vec![RowRef::Message(msg("acct-1", "d1"))],
        action: BulkAction::Archive,
    })
    .await;

    assert_eq!(
        edits.lock().unwrap().len(),
        1,
        "the user named the draft itself"
    );
}

fn item(menu: &RowMenu, action: RowAction) -> (bool, Option<(String, MessageKind)>) {
    let item = menu
        .sections
        .iter()
        .flatten()
        .find(|item| item.action == action)
        .unwrap_or_else(|| panic!("no {action:?} in {menu:?}"));
    (
        item.enabled,
        item.target.as_ref().map(|m| (m.key.clone(), m.kind)),
    )
}

#[tokio::test]
async fn the_menu_answers_a_conversation_from_the_published_snapshot() {
    // Newest is the owner's reply, filed only in Sent: what Reply answers, and what the
    // folder-resolved Archive can still take the received side of.
    let (app, _) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("s1", "sent", 10, MessageKind::Sent, ""),
    ])
    .await;

    let menu = app.row_menu(&conversation());
    assert_eq!(
        item(&menu, RowAction::Reply),
        (true, Some(("s1".to_owned(), MessageKind::Sent)))
    );
    assert_eq!(
        item(&menu, RowAction::Flag),
        (true, Some(("r1".to_owned(), MessageKind::Received)))
    );
    assert_eq!(item(&menu, RowAction::Archive), (true, None));
}

/// The live path rebuilds rows from the cached list and reads no folders, so it must carry over
/// where each account's moves land; without them the menu would disable every move and report
/// until the authoritative rebuild lands.
#[tokio::test]
async fn a_live_commit_keeps_where_the_menu_s_moves_land() {
    let received = mail("r1", "a", 0, MessageKind::Received, "");
    let (app, _) = conversation_app(vec![
        received.clone(),
        mail("s1", "sent", 10, MessageKind::Sent, ""),
    ])
    .await;

    let account = engine_api::AccountId::try_from("acct-1").unwrap();
    app.apply_live_mail_delta(&account, &[received], &[], 0);

    let menu = app.row_menu(&conversation());
    assert_eq!(item(&menu, RowAction::Archive), (true, None));
}

#[tokio::test]
async fn the_menu_will_not_answer_a_draft() {
    let (app, _) = conversation_app(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("d1", "a", 20, MessageKind::Draft, ""),
    ])
    .await;

    let menu = app.row_menu(&conversation());
    assert_eq!(
        item(&menu, RowAction::Open),
        (true, Some(("d1".to_owned(), MessageKind::Draft)))
    );
    assert!(!item(&menu, RowAction::Reply).0);
}

#[tokio::test]
async fn an_account_with_no_archive_offers_archive_disabled() {
    let provider = FakeProvider::with(vec![
        mail("r1", "a", 0, MessageKind::Received, ""),
        mail("r2", "a", 10, MessageKind::Received, ""),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;

    let menu = app.row_menu(&conversation());
    assert!(!item(&menu, RowAction::Archive).0, "nowhere to archive to");
    assert!(!item(&menu, RowAction::Delete).0, "nor a Trash");
}
