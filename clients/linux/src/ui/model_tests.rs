//! Which message a conversation opens, and which conversation stays expanded.

use std::collections::HashSet;

use mailcal_bindings::{SelectedRow, ThreadMessage, ThreadRow};

use super::{kept_expanded, representative};
use crate::ui::mailbox::ThreadKey;

fn message(key: &str, draft: bool) -> ThreadMessage {
    ThreadMessage {
        avatar: super::blank_avatar(),
        account: "acct".to_owned(),
        key: key.to_owned(),
        from: format!("from {key}"),
        date: "2026-07-20".to_owned(),
        preview: String::new(),
        unread: false,
        flagged: false,
        outgoing: false,
        draft,
        has_attachment: false,
    }
}

fn conversation(latest_key: &str, messages: Vec<ThreadMessage>) -> ThreadRow {
    ThreadRow {
        avatar: super::blank_avatar(),
        account: "acct".to_owned(),
        thread_id: "thread".to_owned(),
        latest_key: latest_key.to_owned(),
        subject: "Contribution for next year".to_owned(),
        latest_from: String::new(),
        latest_date: String::new(),
        message_count: u32::try_from(messages.len()).expect("small"),
        unread_count: 0,
        flagged: false,
        has_attachment: false,
        preview: String::new(),
        messages,
    }
}

/// Selecting a conversation opens its newest message whatever its kind, so a draft newest on a
/// conversation in the Inbox resumes in a composer; in Drafts it is the draft the row summarises
/// (`docs/row-actions.md`, rule 2; `docs/drafts.md`).
#[test]
fn a_conversation_opens_its_newest_message_and_says_whether_it_is_a_draft() {
    let row = conversation(
        "received",
        vec![message("draft", true), message("received", false)],
    );
    let (opened, draft) = representative(&row, false);
    assert_eq!((opened.key.as_str(), draft), ("draft", true));
    assert_eq!(opened.subject, "Contribution for next year");
    assert_eq!(opened.from, "from draft");

    let row = conversation(
        "older-draft",
        vec![message("received", false), message("older-draft", true)],
    );
    assert_eq!(representative(&row, false).0.key, "received");
    let (opened, draft) = representative(&row, true);
    assert_eq!((opened.key.as_str(), draft), ("older-draft", true));
}

fn thread(id: &str) -> SelectedRow {
    SelectedRow::Thread {
        account: "acct".to_owned(),
        thread_id: id.to_owned(),
    }
}

/// One conversation is expanded: the one holding the selection (`docs/row-actions.md`, rule 8).
#[test]
fn leaving_a_conversation_folds_it_and_a_selection_of_several_changes_nothing() {
    let open = HashSet::from([ThreadKey::new("acct", "a"), ThreadKey::new("acct", "b")]);

    assert_eq!(
        kept_expanded(&open, &[thread("a")]),
        HashSet::from([ThreadKey::new("acct", "a")]),
        "picking one conversation keeps it alone open, even one opened by its chevron"
    );
    assert!(
        kept_expanded(&open, &[thread("c")]).is_empty(),
        "picking another collapses every other one; the click itself opens the new one"
    );
    let message = SelectedRow::Message {
        account: "acct".to_owned(),
        key: "m".to_owned(),
    };
    assert!(kept_expanded(&open, &[message]).is_empty());
    assert_eq!(kept_expanded(&open, &[thread("a"), thread("c")]), open);
    assert_eq!(
        kept_expanded(&open, &[]),
        open,
        "an emptied selection leaves nothing"
    );
    // Same id, another account: the unified list holds both.
    let other = SelectedRow::Thread {
        account: "other".to_owned(),
        thread_id: "a".to_owned(),
    };
    assert!(kept_expanded(&open, &[other]).is_empty());
}
