//! Where the reading pane lands when the message in it leaves the list.

use super::*;
use crate::{
    row_menu::tests::{mail, threaded_in},
    row_reach::MessageKind,
};

fn landed(stop: Option<MenuMessage>) -> Option<(String, MessageKind)> {
    stop.map(|stop| (stop.key, stop.kind))
}

fn at(key: &str, kind: MessageKind) -> (String, MessageKind) {
    (key.to_owned(), kind)
}

/// The Inbox: a message, a conversation whose newest is the owner's unfinished reply, and an
/// older message below it.
fn inbox() -> MailboxListSnapshot {
    threaded_in(
        "inbox",
        &[
            mail("above", "inbox", 30, "ta", MessageKind::Received),
            mail("r1", "inbox", 5, "t", MessageKind::Received),
            mail("s1", "sent", 10, "t", MessageKind::Sent),
            mail("d1", "drafts", 20, "t", MessageKind::Draft),
            mail("below", "inbox", 1, "tb", MessageKind::Received),
        ],
    )
}

/// A collapsed conversation is the message clicking it opens, its newest, and that one is a
/// draft, so the pane resumes it in a composer as a click would.
#[test]
fn the_pane_lands_on_what_clicking_the_next_row_opens_and_a_draft_says_so() {
    let snapshot = inbox();
    let collapsed = |_: &ThreadRow| false;
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "above", collapsed)),
        Some(at("d1", MessageKind::Draft))
    );
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "d1", collapsed)),
        Some(at("below", MessageKind::Received))
    );
}

/// An expanded conversation offers each of its messages, newest first, its Sent copy included.
#[test]
fn an_expanded_conversation_offers_each_of_its_messages() {
    let snapshot = inbox();
    let expanded = |thread: &ThreadRow| thread.thread_id == "t";
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "d1", expanded)),
        Some(at("s1", MessageKind::Sent))
    );
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "r1", expanded)),
        Some(at("below", MessageKind::Received))
    );
}

/// The last message falls back to the one above; the only one, or one the list no longer shows,
/// lands nowhere, and the pane empties.
#[test]
fn the_end_of_the_list_falls_back_upwards_and_nothing_left_lands_nowhere() {
    let snapshot = inbox();
    let collapsed = |_: &ThreadRow| false;
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "below", collapsed)),
        Some(at("d1", MessageKind::Draft))
    );
    assert_eq!(stop_after_removing(&snapshot, "a", "gone", collapsed), None);
    assert_eq!(
        stop_after_removing(&snapshot, "b", "above", collapsed),
        None
    );

    let alone = threaded_in(
        "inbox",
        &[mail("only", "inbox", 1, "t", MessageKind::Received)],
    );
    assert_eq!(stop_after_removing(&alone, "a", "only", collapsed), None);
}

/// In Drafts a conversation is the draft its row summarises, even when a reply received after it
/// is the conversation's newest message (`docs/drafts.md`).
#[test]
fn in_drafts_a_conversation_is_the_draft_its_row_summarises() {
    let mut snapshot = threaded_in(
        "drafts",
        &[
            mail("newer-draft", "drafts", 30, "u", MessageKind::Draft),
            mail("r1", "inbox", 0, "t", MessageKind::Received),
            mail("d1", "drafts", 10, "t", MessageKind::Draft),
            mail("r2", "inbox", 20, "t", MessageKind::Received),
        ],
    );
    // Set by the folder's role, which the fixture's folders do not carry.
    snapshot.showing_drafts = true;
    assert_eq!(
        landed(stop_after_removing(&snapshot, "a", "newer-draft", |_| {
            false
        })),
        Some(at("d1", MessageKind::Draft))
    );
}
