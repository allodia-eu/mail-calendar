//! The Outbox's rules, at the layer a test can reach them.
//!
//! The projection first, because its failures are silent on screen: a state mapped to the wrong
//! word tells someone their message is waiting when it is already on its way, and a row that
//! offers "Send now" on a message that may have been delivered is how it arrives twice. Neither
//! looks wrong.
//!
//! The GTK half is functions rather than `#[test]`s, called by the crate's single `gtk::init`
//! test, for the reason [`super::super::mailbox_thread_tests`] gives.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::{AccountRow, MailboxListSnapshot, QueuedRow, QueuedState};

use super::{
    account_email, is_actionable, recipients_line, render, row_a11y, state_label, subject_line,
};
use crate::{
    l10n,
    ui::{AppInput, mailbox::tests::rendered_labels, model::empty_mailbox},
};

fn queued(op: u64, to: &str, subject: &str, state: QueuedState) -> QueuedRow {
    QueuedRow {
        account: "acct-1".to_owned(),
        op,
        to: to.to_owned(),
        subject: subject.to_owned(),
        state,
        attempts: 1,
        detail: None,
    }
}

fn with_outbox(outbox: Vec<QueuedRow>) -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![AccountRow {
            id: "acct-1".to_owned(),
            email: "eva.jansen@example.test".to_owned(),
            name: String::new(),
            expanded: true,
        }],
        showing_outbox: true,
        outbox,
        ..empty_mailbox()
    }
}

/// **The rule the whole surface turns on.** Only a message still waiting may be acted on: one in
/// flight cannot be called back, and one whose delivery could not be confirmed may already be in
/// front of its recipients (`docs/sending.md`).
#[test]
fn only_a_message_still_waiting_offers_its_three_actions() {
    assert!(is_actionable(QueuedState::Waiting));
    assert!(!is_actionable(QueuedState::Sending));
    assert!(!is_actionable(QueuedState::Unconfirmed));
}

/// Each state says what it is, in the app's own words, and no two say the same thing.
#[test]
fn every_queued_state_is_written_as_itself() {
    assert_eq!(state_label(QueuedState::Waiting), l10n::outbox_waiting());
    assert_eq!(state_label(QueuedState::Sending), l10n::outbox_sending());
    assert_eq!(
        state_label(QueuedState::Unconfirmed),
        l10n::outbox_unconfirmed()
    );
    let words = [
        state_label(QueuedState::Waiting),
        state_label(QueuedState::Sending),
        state_label(QueuedState::Unconfirmed),
    ];
    for (index, word) in words.iter().enumerate() {
        assert!(
            !words[index + 1..].contains(word),
            "two states may not read as the same sentence: {word}"
        );
    }
}

/// Neither line is ever left blank: an empty one reads as a rendering fault rather than as a
/// fact, and both genuinely come back empty (a Bcc-only message, and mail with no subject).
#[test]
fn a_row_with_nothing_to_say_still_says_something() {
    let bcc_only = queued(1, "", "", QueuedState::Waiting);
    assert_eq!(
        recipients_line(&bcc_only, "eva.jansen@example.test"),
        "eva.jansen@example.test"
    );
    assert_eq!(subject_line(&bcc_only), l10n::mail_no_subject());

    let ordinary = queued(2, "ada@example.test", "Lunch", QueuedState::Waiting);
    assert_eq!(
        recipients_line(&ordinary, "eva.jansen@example.test"),
        "ada@example.test"
    );
    assert_eq!(subject_line(&ordinary), "Lunch");
}

/// An account id is resolved to the address the user knows it by, and a queued send whose
/// account has since been removed still names something rather than nothing.
#[test]
fn a_row_names_the_account_it_would_go_out_from() {
    let snapshot = with_outbox(vec![queued(
        1,
        "ada@example.test",
        "Hi",
        QueuedState::Waiting,
    )]);
    assert_eq!(
        account_email(&snapshot, "acct-1"),
        "eva.jansen@example.test"
    );
    assert_eq!(account_email(&snapshot, "acct-gone"), "acct-gone");
}

/// The tooltip carries the same four things the row draws, in the order it draws them.
#[test]
fn the_whole_row_reads_as_one_sentence() {
    assert_eq!(
        row_a11y(
            "ada@example.test",
            "Lunch",
            QueuedState::Waiting,
            "eva.jansen@example.test"
        ),
        format!(
            "ada@example.test. Lunch. {}. eva.jansen@example.test",
            l10n::outbox_waiting()
        )
    );
}

/// Rule 18: the pane's Outbox row exists only while something is in it, and its badge counts
/// what is waiting to be **sent**, never what is unread.
pub(crate) fn the_pane_row_appears_only_while_something_is_waiting() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let list = gtk::ListBox::new();
    let empty = empty_mailbox();
    crate::ui::folder_pane::render(
        &list,
        &empty,
        &HashSet::default(),
        &crate::ui::folder_actions::name_check(None),
        &sender,
    );
    let labels = rendered_labels(list.clone().upcast_ref());
    assert!(
        !labels.iter().any(|label| label == l10n::folder_outbox()),
        "an Outbox nobody has anything in is furniture that only ever says zero"
    );

    let waiting = with_outbox(vec![
        queued(1, "ada@example.test", "Lunch", QueuedState::Waiting),
        queued(2, "bob@example.test", "Notes", QueuedState::Waiting),
    ]);
    let list = gtk::ListBox::new();
    crate::ui::folder_pane::render(
        &list,
        &waiting,
        &HashSet::default(),
        &crate::ui::folder_actions::name_check(None),
        &sender,
    );
    let labels = rendered_labels(list.clone().upcast_ref());
    assert!(
        labels.iter().any(|label| label == l10n::folder_outbox()),
        "two messages are waiting, so the row is on screen: {labels:?}"
    );
    let badge = crate::ui::mailbox::test_tree::labels(list.clone().upcast_ref())
        .into_iter()
        .find(|label| label.text() == "2")
        .expect("the badge counts what is waiting");
    // The bare number reads as a position in a list, so the badge carries its own sentence;
    // "2 unread" would be the wrong one, because nobody has received these.
    assert_eq!(
        badge.tooltip_text().map(|text| text.to_string()).as_deref(),
        Some(l10n::a11y_outbox_count(2).as_str())
    );
}

/// The Outbox holds the pane highlight while its list is on screen.
///
/// Neither selection scalar can say this: `selected_account` and `selected` are both `None` on
/// the Outbox **and** on the unified inbox, so a pane that guessed would light All Inboxes.
pub(crate) fn the_pane_highlights_the_outbox_rather_than_everyones_inbox() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let snapshot = with_outbox(vec![queued(
        1,
        "ada@example.test",
        "Lunch",
        QueuedState::Waiting,
    )]);
    let list = gtk::ListBox::new();
    crate::ui::folder_pane::render(
        &list,
        &snapshot,
        &HashSet::default(),
        &crate::ui::folder_actions::name_check(None),
        &sender,
    );
    let selected = list.selected_row().expect("the Outbox row is selected");
    assert_eq!(selected.index(), 0, "and it is the row above the trees");

    // The same pane, the same two null scalars, with the Outbox *not* showing: the unified
    // inbox takes the highlight back.
    let unified = MailboxListSnapshot {
        showing_outbox: false,
        ..snapshot
    };
    let list = gtk::ListBox::new();
    crate::ui::folder_pane::render(
        &list,
        &unified,
        &HashSet::default(),
        &crate::ui::folder_actions::name_check(None),
        &sender,
    );
    let selected = list.selected_row().expect("All Inboxes is selected");
    assert_eq!(
        selected.index(),
        2,
        "beneath the Outbox row and the All Accounts heading"
    );
}

/// Opening the Outbox moves the pane highlight, which the selection key has to notice.
///
/// It caches what it last applied, and the two scalars it used to cache are `None` both on the
/// Outbox and on the unified inbox: without `showing_outbox` in that key, opening the Outbox is
/// "no change" and the highlight stays where it was.
pub(crate) fn opening_the_outbox_moves_a_highlight_the_selection_cache_would_have_held() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let queued_row = || queued(1, "ada@example.test", "Lunch", QueuedState::Waiting);
    let snapshot = with_outbox(vec![queued_row()]);
    // The same pane, the same two null scalars, with the Outbox not showing.
    let unified = MailboxListSnapshot {
        showing_outbox: false,
        ..with_outbox(vec![queued_row()])
    };
    let list = gtk::ListBox::new();
    crate::ui::folder_pane::render(
        &list,
        &unified,
        &HashSet::default(),
        &crate::ui::folder_actions::name_check(None),
        &sender,
    );
    let mut selection = crate::ui::folder_pane::FolderPaneSelection::default();
    selection.sync(&list, &unified);
    assert_eq!(
        list.selected_row().map(|row| row.index()),
        Some(2),
        "All Inboxes to begin with"
    );

    selection.sync(&list, &snapshot);
    assert_eq!(
        list.selected_row().map(|row| row.index()),
        Some(0),
        "and the Outbox once its list is what is on screen"
    );
}

/// Every row names its recipients, its subject and its state; a message that cannot be acted on
/// carries no menu at all, which is what stops it being offered a retry.
pub(crate) fn a_queued_row_states_its_case_and_offers_a_retry_only_when_one_is_safe() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let snapshot = with_outbox(vec![
        queued(1, "ada@example.test", "Lunch", QueuedState::Waiting),
        queued(2, "bob@example.test", "Notes", QueuedState::Unconfirmed),
    ]);
    let list = gtk::ListBox::new();
    render(&list, &snapshot, &sender);
    let labels = rendered_labels(list.clone().upcast_ref());
    for expected in [
        "Lunch",
        "ada@example.test",
        l10n::outbox_waiting(),
        "Notes",
        "bob@example.test",
        l10n::outbox_unconfirmed(),
        "eva.jansen@example.test",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "the list must say {expected:?}: {labels:?}"
        );
    }

    // Through the accessibility tree, not just the labels: a presentational container prunes
    // its children out of it, so a row can show its state and account on screen while a screen
    // reader hears neither.
    for spoken in [l10n::outbox_waiting(), "eva.jansen@example.test"] {
        assert!(
            crate::ui::mailbox::test_tree::labels(list.clone().upcast_ref())
                .iter()
                .any(|label| label.text() == spoken
                    && label.accessible_role() != gtk::AccessibleRole::Presentation),
            "{spoken:?} must reach assistive technology, not only the screen"
        );
    }

    let waiting = list.row_at_index(0).expect("the waiting row");
    let unconfirmed = list.row_at_index(1).expect("the unconfirmed row");
    assert!(
        has_menu(waiting.clone().upcast_ref()),
        "a message still waiting offers Send now, Edit and Cancel"
    );
    assert!(
        !has_menu(unconfirmed.clone().upcast_ref()),
        "one that may already have been delivered offers nothing: asking again is how it \
         arrives twice"
    );
}

/// The list says so when there is nothing in it, which is reachable without navigating: cancel
/// the last row and the list the user is looking at empties under them.
pub(crate) fn an_emptied_outbox_says_so_rather_than_going_blank() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let list = gtk::ListBox::new();
    render(&list, &with_outbox(Vec::new()), &sender);
    let labels = rendered_labels(list.clone().upcast_ref());
    assert!(
        labels.iter().any(|label| label == l10n::outbox_empty()),
        "an empty list with no words in it reads as a failure to load: {labels:?}"
    );
}

/// Whether a row carries the overflow button the three actions live behind.
fn has_menu(row: &gtk::Widget) -> bool {
    let mut found = false;
    let mut child = row.first_child();
    while let Some(widget) = child {
        if widget
            .downcast_ref::<gtk::Button>()
            .and_then(gtk::prelude::ButtonExt::icon_name)
            .is_some_and(|name| name == crate::ui::icons::MORE)
        {
            return true;
        }
        found |= has_menu(&widget);
        child = widget.next_sibling();
    }
    found
}
