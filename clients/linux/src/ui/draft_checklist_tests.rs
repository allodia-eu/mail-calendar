//! A drafted reply's card, pinned without a window: the items in the core's order, a fill-in item
//! that follows the reply and nothing else, the others ticked by the person, and Send asking once
//! per composer while anything is open.

use mailcal_bindings::{DraftTask, DraftTaskKind};

use super::{DraftChecklist, placeholders_answer, placeholders_script};
use crate::l10n;

fn task(kind: DraftTaskKind, text: &str) -> DraftTask {
    DraftTask {
        kind,
        text: text.to_owned(),
    }
}

fn tasks() -> Vec<DraftTask> {
    vec![
        task(DraftTaskKind::FillIn, "[date]"),
        task(DraftTaskKind::FillIn, "[time]"),
        task(DraftTaskKind::Attach, "Attach the agenda"),
        task(DraftTaskKind::Do, "Book the room"),
    ]
}

fn drafted(tasks: &[DraftTask], attachments: usize) -> DraftChecklist {
    let mut checklist = DraftChecklist::default();
    checklist.show("Bob asks when you can meet.", tasks, attachments);
    checklist
}

fn ticks(checklist: &DraftChecklist) -> Vec<bool> {
    checklist.items().iter().map(|item| item.ticked).collect()
}

#[test]
fn items_keep_the_cores_order_and_a_later_draft_replaces_them() {
    let mut checklist = drafted(&tasks(), 0);
    let texts = checklist
        .items()
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        texts,
        ["[date]", "[time]", "Attach the agenda", "Book the room"]
    );
    assert_eq!(
        checklist.items()[0].title(),
        l10n::composer_task_fill_in("[date]")
    );
    assert_eq!(checklist.items()[3].title(), "Book the room");
    checklist.toggle(3);
    let first = checklist.draft();
    checklist.show("", &[task(DraftTaskKind::Do, "Call Bob")], 0);
    assert_eq!(checklist.items().len(), 1);
    assert_eq!(checklist.items()[0].text, "Call Bob");
    assert_eq!(checklist.open_count(), 1);
    assert!(checklist.summary().is_empty());
    assert_ne!(checklist.draft(), first);
}

#[test]
fn a_draft_with_nothing_to_say_shows_no_card() {
    assert!(DraftChecklist::default().is_empty());
    let mut checklist = DraftChecklist::default();
    checklist.show("", &[], 0);
    assert!(checklist.is_empty());
    checklist.show("Bob asks for the slides.", &[], 0);
    assert!(!checklist.is_empty());
}

/// Ticked exactly while the placeholder is gone, so an undo opens the item again.
#[test]
fn a_fill_in_item_follows_what_is_left_in_the_reply() {
    let mut checklist = drafted(&tasks(), 0);
    assert_eq!(checklist.placeholders(), ["[date]", "[time]"]);
    assert!(checklist.awaits_placeholders());
    checklist.placeholders_left(&["[time]".to_owned()]);
    assert_eq!(ticks(&checklist), [true, false, false, false]);
    checklist.placeholders_left(&[]);
    assert!(!checklist.awaits_placeholders());
    assert!(
        checklist.follows(),
        "the attach item still follows the files"
    );
    checklist.placeholders_left(&["[date]".to_owned()]);
    assert_eq!(ticks(&checklist), [false, true, false, false]);
    assert!(
        checklist.awaits_placeholders(),
        "a reopened item is followed again"
    );
}

#[test]
fn only_the_items_the_editor_cannot_see_are_ticked_by_hand() {
    let mut checklist = drafted(&tasks(), 0);
    checklist.toggle(0);
    assert!(!checklist.items()[0].ticked);
    checklist.toggle(2);
    checklist.toggle(3);
    assert!(checklist.items()[2].ticked && checklist.items()[3].ticked);
    checklist.toggle(3);
    assert!(!checklist.items()[3].ticked);
    checklist.placeholders_left(&[]);
    assert!(
        checklist.items()[2].ticked,
        "the reply leaves an attach item alone"
    );
    checklist.toggle(9);
    assert_eq!(checklist.items().len(), 4, "no such item, nothing changes");
}

/// Which file answers which item cannot be told, so an attach item ticks only once there is a new
/// file for every one of them.
#[test]
fn attach_items_tick_once_each_has_a_new_file() {
    let attaching = [
        task(DraftTaskKind::Attach, "Attach the agenda"),
        task(DraftTaskKind::Attach, "Attach the minutes"),
        task(DraftTaskKind::Do, "Book the room"),
    ];
    let mut checklist = drafted(&attaching, 1);
    checklist.attachments_changed(2);
    assert_eq!(ticks(&checklist), [false, false, false]);
    checklist.attachments_changed(3);
    assert_eq!(ticks(&checklist), [true, true, false]);
    assert!(!checklist.follows(), "nothing left that ticks itself");

    let mut none = drafted(&[task(DraftTaskKind::Do, "Book the room")], 0);
    none.attachments_changed(4);
    assert!(!none.items()[0].ticked);
}

#[test]
fn the_open_count_is_every_unticked_item() {
    let mut checklist = drafted(&tasks(), 0);
    assert_eq!(checklist.open_count(), 4);
    checklist.placeholders_left(&["[time]".to_owned()]);
    checklist.toggle(2);
    assert_eq!(checklist.open_count(), 2);
}

/// Either answer ends the asking for the composer, a later draft included; and with nothing open
/// it never asks at all.
#[test]
fn send_asks_once_and_only_while_something_is_open() {
    let mut checklist = drafted(&tasks(), 0);
    assert!(checklist.asks_before_send());
    checklist.send_asked();
    assert!(!checklist.asks_before_send());
    checklist.show("", &tasks(), 0);
    assert!(checklist.has_asked() && !checklist.asks_before_send());

    let mut done = drafted(&[task(DraftTaskKind::Do, "Book the room")], 0);
    done.toggle(0);
    assert!(!done.asks_before_send());
    assert!(!DraftChecklist::default().asks_before_send());
}

/// A placeholder is data on its way to the editor, and anything but a list of strings on its way
/// back is no answer.
#[test]
fn the_editor_is_asked_with_a_json_list_and_answers_with_one() {
    let script = placeholders_script(&["[date]".to_owned(), "[\"x\"]".to_owned()]);
    let argument = script
        .strip_prefix("window.composerPlaceholdersLeft(")
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("one call");
    let parsed: Vec<String> = serde_json::from_str(argument).expect("a JSON list");
    assert_eq!(parsed, ["[date]", "[\"x\"]"]);

    assert_eq!(
        placeholders_answer(Some("[\"[time]\"]")),
        Some(vec!["[time]".to_owned()])
    );
    assert_eq!(placeholders_answer(Some("[]")), Some(Vec::new()));
    assert_eq!(placeholders_answer(Some("undefined")), None);
    assert_eq!(placeholders_answer(Some("{\"a\":1}")), None);
    assert_eq!(placeholders_answer(None), None);
}
