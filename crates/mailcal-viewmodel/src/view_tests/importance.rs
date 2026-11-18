//! The importance a row draws: a message's own, and a conversation's from its messages
//! (`docs/importance.md`).

use engine_api::Importance;

use super::*;

fn with_importance(mut message: Message, importance: Importance) -> Message {
    message.envelope.importance = importance;
    message
}

fn thread_importance(messages: &[AccountMessage]) -> Importance {
    let snapshot = build(
        messages,
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    let SnapshotRow::Thread(thread) = &snapshot.rows[0] else {
        panic!("expected a thread row");
    };
    thread.importance
}

#[test]
fn a_flat_row_carries_its_messages_importance() {
    let urgent = with_importance(message("m1", "Urgent", 1, None), Importance::High);
    assert_eq!(flat_row(&at("a", &urgent)).importance, Importance::High);
    let plain = message("m2", "Plain", 2, None);
    assert_eq!(flat_row(&at("a", &plain)).importance, Importance::Normal);
}

#[test]
fn a_conversation_is_high_when_any_message_is() {
    // The urgent message is the older one: the replies after it do not hide it.
    let latest = message("m1", "Re: Urgent", 30, Some("t"));
    let urgent = with_importance(message("m2", "Urgent", 0, Some("t")), Importance::High);
    assert_eq!(
        thread_importance(&[at("a", &latest), at("a", &urgent)]),
        Importance::High
    );
}

#[test]
fn a_conversation_is_low_only_when_every_message_is() {
    let low =
        |id, minute| with_importance(message(id, "Later", minute, Some("t")), Importance::Low);
    assert_eq!(
        thread_importance(&[at("a", &low("m1", 30)), at("a", &low("m2", 0))]),
        Importance::Low
    );
    let normal = message("m3", "Re: Later", 40, Some("t"));
    assert_eq!(
        thread_importance(&[at("a", &normal), at("a", &low("m1", 30))]),
        Importance::Normal
    );
}

#[test]
fn an_expanded_conversation_lists_each_messages_own_importance() {
    let latest = message("m1", "Re: Urgent", 30, Some("t"));
    let urgent = with_importance(message("m2", "Urgent", 0, Some("t")), Importance::High);
    let snapshot = build(
        &[at("a", &latest), at("a", &urgent)],
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    let importances: Vec<_> = thread_messages(&snapshot)
        .iter()
        .map(|message| message.importance)
        .collect();
    assert_eq!(importances, [Importance::Normal, Importance::High]);
}
