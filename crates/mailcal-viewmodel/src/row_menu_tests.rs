//! The row menu over the three conversations `docs/row-actions.md` is written against: one whose
//! newest message was received, one whose newest was sent, and one whose newest is a draft.

use std::sync::Arc;

use engine_api::{Keyword, MailListRow, Message, SystemKeyword, ThreadRef};
use engine_core::{
    ids::{AccountId, MailboxId, MessageId, ThreadId},
    membership::Memberships,
};

use super::*;
use crate::view::{AccountMessage, ViewMode, build};

/// One message of thread `thread`, filed in `folder`, at minute `minute`, read and unflagged.
fn mail(key: &str, folder: &str, minute: u8, thread: &str, kind: MessageKind) -> AccountMessage {
    marked(key, folder, minute, thread, kind, "")
}

/// [`mail`], with `marks` naming what differs: `u` for unread, `f` for flagged.
fn marked(
    key: &str,
    folder: &str,
    minute: u8,
    thread: &str,
    kind: MessageKind,
    marks: &str,
) -> AccountMessage {
    let mut message = Message::new(
        MessageId::try_from(key).unwrap(),
        Memberships::of_one(MailboxId::try_from(folder).unwrap()),
    );
    message.envelope.subject = Some("Test thread".to_owned());
    message.received_at = Some(format!("2026-10-30T09:{minute:02}:00Z").parse().unwrap());
    message.thread = Some(ThreadRef::provider_assigned(
        ThreadId::try_from(thread).unwrap(),
    ));
    if !marks.contains('u') {
        message
            .keywords
            .insert(Keyword::system(SystemKeyword::Seen));
    }
    if marks.contains('f') {
        message
            .keywords
            .insert(Keyword::system(SystemKeyword::Flagged));
    }
    if kind == MessageKind::Draft {
        message
            .keywords
            .insert(Keyword::system(SystemKeyword::Draft));
    }
    AccountMessage {
        row: Arc::new(MailListRow::project(
            &AccountId::try_from("a").unwrap(),
            &message,
        )),
        in_scope: folder == "inbox",
        outgoing: kind == MessageKind::Sent,
    }
}

/// Account `a`'s folders, every one of them present.
fn places() -> AccountPlaces {
    AccountPlaces {
        account: "a".to_owned(),
        inbox: Some("inbox".to_owned()),
        archive: Some("archive".to_owned()),
        trash: Some("trash".to_owned()),
        junk: Some("junk".to_owned()),
        kept: vec!["sent".to_owned()],
    }
}

fn threaded(messages: &[AccountMessage]) -> MailboxListSnapshot {
    threaded_in("inbox", messages)
}

/// The threaded list of `folder`: a conversation is listed when one of its messages is there.
fn threaded_in(folder: &str, messages: &[AccountMessage]) -> MailboxListSnapshot {
    let messages: Vec<AccountMessage> = messages
        .iter()
        .cloned()
        .map(|mut item| {
            item.in_scope = item
                .row
                .mailboxes
                .iter()
                .any(|id| id.key().as_str() == folder);
            item
        })
        .collect();
    let mut snapshot = build(
        &messages,
        &[],
        &[],
        vec![],
        Some("a"),
        Some(folder),
        ViewMode::Threaded,
        usize::MAX,
    );
    snapshot.places = vec![places()];
    snapshot
}

fn thread(id: &str) -> MenuRow {
    MenuRow::Thread {
        account: "a".to_owned(),
        thread_id: id.to_owned(),
    }
}

fn message(key: &str) -> MenuRow {
    MenuRow::Message {
        account: "a".to_owned(),
        key: key.to_owned(),
    }
}

/// The item for `action`, which the menu must hold.
fn find(menu: &RowMenu, action: RowAction) -> &RowMenuItem {
    menu.sections
        .iter()
        .flatten()
        .find(|item| item.action == action)
        .unwrap_or_else(|| panic!("no {action:?} in {menu:?}"))
}

fn has(menu: &RowMenu, action: RowAction) -> bool {
    menu.sections
        .iter()
        .flatten()
        .any(|item| item.action == action)
}

fn target(menu: &RowMenu, action: RowAction) -> Option<(&str, MessageKind)> {
    find(menu, action)
        .target
        .as_ref()
        .map(|m| (m.key.as_str(), m.kind))
}

fn enabled(menu: &RowMenu, action: RowAction) -> bool {
    find(menu, action).enabled
}

/// Received, then the owner's reply (filed in Sent), then a received answer, unread.
fn latest_received() -> Vec<AccountMessage> {
    vec![
        mail("r1", "inbox", 0, "t", MessageKind::Received),
        mail("s1", "sent", 10, "t", MessageKind::Sent),
        marked("r2", "inbox", 20, "t", MessageKind::Received, "u"),
    ]
}

/// Received, then the owner's reply, which is the newest.
fn latest_sent() -> Vec<AccountMessage> {
    vec![
        mail("r1", "inbox", 0, "t", MessageKind::Received),
        mail("s1", "sent", 10, "t", MessageKind::Sent),
    ]
}

/// Received, the owner's reply, and a draft answering it, which is the newest.
fn latest_draft() -> Vec<AccountMessage> {
    vec![
        mail("r1", "inbox", 0, "t", MessageKind::Received),
        mail("s1", "sent", 10, "t", MessageKind::Sent),
        mail("d1", "drafts", 20, "t", MessageKind::Draft),
    ]
}

#[test]
fn a_conversation_opens_and_answers_its_newest_message_whatever_its_kind() {
    for (messages, newest, kind) in [
        (latest_received(), "r2", MessageKind::Received),
        (latest_sent(), "s1", MessageKind::Sent),
        (latest_draft(), "d1", MessageKind::Draft),
    ] {
        let menu = row_menu(&threaded(&messages), &[thread("t")], false);
        assert_eq!(target(&menu, RowAction::Open), Some((newest, kind)));
        assert_eq!(target(&menu, RowAction::Reply), Some((newest, kind)));
        assert_eq!(target(&menu, RowAction::Forward), Some((newest, kind)));
    }
}

#[test]
fn a_draft_cannot_be_answered_and_the_message_beside_it_can() {
    let snapshot = threaded(&latest_draft());
    let menu = row_menu(&snapshot, &[thread("t")], false);
    for action in [RowAction::Reply, RowAction::ReplyAll, RowAction::Forward] {
        assert!(!enabled(&menu, action), "{action:?} on a draft");
    }
    assert!(enabled(&menu, RowAction::Open), "a draft still opens");

    let sent = row_menu(&snapshot, &[message("s1")], false);
    assert!(enabled(&sent, RowAction::Reply));
    assert_eq!(
        target(&sent, RowAction::Reply),
        Some(("s1", MessageKind::Sent))
    );
}

#[test]
fn flag_and_report_are_about_the_newest_received_message() {
    let menu = row_menu(&threaded(&latest_sent()), &[thread("t")], false);
    assert_eq!(
        target(&menu, RowAction::Flag),
        Some(("r1", MessageKind::Received))
    );
    assert_eq!(
        target(&menu, RowAction::MarkAsSpam),
        Some(("r1", MessageKind::Received))
    );
    assert!(enabled(&menu, RowAction::MarkAsSpam));
}

#[test]
fn read_follows_the_received_messages_and_acts_on_the_rows() {
    let menu = row_menu(&threaded(&latest_received()), &[thread("t")], false);
    assert!(enabled(&menu, RowAction::MarkRead), "r2 is unread");
    assert_eq!(find(&menu, RowAction::MarkRead).target, None);

    let all_read = row_menu(&threaded(&latest_sent()), &[thread("t")], false);
    assert!(enabled(&all_read, RowAction::MarkUnread));

    // The owner's own messages alone carry no read state to change.
    let own = threaded_in(
        "sent",
        &[
            mail("s1", "sent", 0, "t", MessageKind::Sent),
            marked("s2", "sent", 10, "t", MessageKind::Sent, "u"),
        ],
    );
    let menu = row_menu(&own, &[thread("t")], false);
    assert!(!enabled(&menu, RowAction::MarkUnread));
}

#[test]
fn an_older_flag_makes_the_conversation_offer_unflag_over_the_rows() {
    let messages = vec![
        marked("r1", "inbox", 0, "t", MessageKind::Received, "f"),
        mail("r2", "inbox", 20, "t", MessageKind::Received),
    ];
    let snapshot = threaded(&messages);
    let menu = row_menu(&snapshot, &[thread("t")], false);
    assert!(enabled(&menu, RowAction::Unflag));
    assert_eq!(find(&menu, RowAction::Unflag).target, None);
    assert!(!has(&menu, RowAction::Flag));

    // The row shows the flag the menu offers to clear, though its newest message has none.
    let SnapshotRow::Thread(row) = &snapshot.rows[0] else {
        panic!("expected a conversation row");
    };
    assert!(row.flagged);
    assert_eq!(
        row.messages.iter().map(|m| m.flagged).collect::<Vec<_>>(),
        [false, true]
    );
}

#[test]
fn an_unflagged_conversation_shows_no_flag_and_offers_flag() {
    let snapshot = threaded(&latest_received());
    let SnapshotRow::Thread(row) = &snapshot.rows[0] else {
        panic!("expected a conversation row");
    };
    assert!(!row.flagged);
    assert!(has(
        &row_menu(&snapshot, &[thread("t")], false),
        RowAction::Flag
    ));
}

#[test]
fn a_conversation_with_nothing_received_flags_its_newest_message() {
    let own = threaded_in(
        "sent",
        &[
            mail("s1", "sent", 0, "t", MessageKind::Sent),
            mail("s2", "sent", 10, "t", MessageKind::Sent),
        ],
    );
    let menu = row_menu(&own, &[thread("t")], false);
    assert_eq!(
        target(&menu, RowAction::Flag),
        Some(("s2", MessageKind::Sent))
    );
    assert!(!enabled(&menu, RowAction::MarkAsSpam), "nothing to report");
}

#[test]
fn a_conversation_move_leaves_sent_copies_and_drafts_where_they_are() {
    // Every message here stays put, so none of the moves has anything to take.
    let own = threaded_in(
        "archive",
        &[
            mail("r1", "archive", 0, "t", MessageKind::Received),
            mail("s1", "sent", 10, "t", MessageKind::Sent),
            mail("d1", "drafts", 20, "t", MessageKind::Draft),
        ],
    );
    let menu = row_menu(&own, &[thread("t")], false);
    assert!(
        !enabled(&menu, RowAction::Archive),
        "r1 is already archived"
    );
    assert!(
        enabled(&menu, RowAction::Delete),
        "r1 can still go to Trash"
    );

    // The user named the Sent copy itself, so it moves.
    let named = row_menu(&own, &[message("s1")], false);
    assert!(enabled(&named, RowAction::Archive));
    assert_eq!(
        target(&named, RowAction::Archive),
        Some(("s1", MessageKind::Sent))
    );
}

#[test]
fn archive_and_delete_are_disabled_where_nothing_would_move() {
    let snapshot = threaded_in(
        "archive",
        &[
            mail("r1", "archive", 0, "t", MessageKind::Received),
            mail("r2", "archive", 10, "t", MessageKind::Received),
        ],
    );
    let menu = row_menu(&snapshot, &[thread("t")], false);
    assert!(!enabled(&menu, RowAction::Archive));
    assert!(enabled(&menu, RowAction::Delete));

    let mut no_archive = threaded(&latest_received());
    no_archive.places[0].archive = None;
    let menu = row_menu(&no_archive, &[thread("t")], false);
    assert!(
        !enabled(&menu, RowAction::Archive),
        "the account has no Archive"
    );
}

#[test]
fn a_message_in_junk_is_reported_as_not_junk() {
    let snapshot = threaded_in(
        "junk",
        &[
            mail("j1", "junk", 0, "t", MessageKind::Received),
            mail("j2", "junk", 10, "t", MessageKind::Received),
        ],
    );
    let menu = row_menu(&snapshot, &[thread("t")], false);
    assert!(!has(&menu, RowAction::MarkAsSpam));
    assert!(enabled(&menu, RowAction::MarkAsNotSpam));
}

#[test]
fn only_a_desktop_offers_a_window_of_its_own() {
    let snapshot = threaded(&latest_sent());
    assert!(!has(
        &row_menu(&snapshot, &[thread("t")], false),
        RowAction::OpenInWindow
    ));
    let desktop = row_menu(&snapshot, &[thread("t")], true);
    assert_eq!(
        target(&desktop, RowAction::OpenInWindow),
        Some(("s1", MessageKind::Sent))
    );
}

#[test]
fn a_selection_menu_offers_the_batch_over_every_row() {
    let mut messages = latest_received();
    messages.extend([
        mail("x1", "inbox", 1, "x", MessageKind::Received),
        mail("x2", "inbox", 2, "x", MessageKind::Received),
    ]);
    let snapshot = threaded(&messages);
    let menu = row_menu(&snapshot, &[thread("t"), thread("x")], true);
    for action in [RowAction::Open, RowAction::Reply, RowAction::MarkAsSpam] {
        assert!(!has(&menu, action), "{action:?} is about one row");
    }
    assert!(enabled(&menu, RowAction::MarkRead), "t holds unread mail");
    assert!(enabled(&menu, RowAction::Flag));
    assert!(has(&menu, RowAction::MoveToFolder), "one account");
    assert!(
        menu.sections
            .iter()
            .flatten()
            .all(|item| item.target.is_none()),
        "a selection's items act on its rows"
    );
}

#[test]
fn a_row_the_list_no_longer_shows_has_no_menu() {
    let menu = row_menu(&threaded(&latest_sent()), &[thread("gone")], true);
    assert_eq!(menu, RowMenu::default());
}

#[test]
fn a_draft_reads_as_a_draft_even_from_the_owners_address() {
    let draft = mail("d1", "drafts", 0, "t", MessageKind::Draft);
    assert_eq!(MessageKind::of(&draft.row, true), MessageKind::Draft);
}

#[test]
fn an_unread_copy_of_the_owners_own_leaves_the_conversation_read() {
    // Another client filed the owner's reply without the read mark. Nothing on the row reaches
    // it, so it must not bold the row either.
    let snapshot = threaded(&[
        mail("r1", "inbox", 0, "t", MessageKind::Received),
        marked("s1", "sent", 10, "t", MessageKind::Sent, "u"),
    ]);
    let SnapshotRow::Thread(row) = &snapshot.rows[0] else {
        panic!("expected a conversation row");
    };
    assert_eq!(row.unread_count, 0);
    let menu = row_menu(&snapshot, &[thread("t")], false);
    assert!(has(&menu, RowAction::MarkUnread));
}
