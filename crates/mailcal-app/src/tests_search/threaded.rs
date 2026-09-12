//! Search under conversation grouping: rule 9 of `docs/search.md`. A match is shown as the
//! conversation it belongs to, carrying the whole thread, labelled by and ordered on the newest
//! message that matched.

use std::sync::{Arc, Mutex};

use engine_core::mail::Message;
use mailcal_viewmodel::{MailboxListSnapshot, SnapshotRow, ViewMode};

use super::{
    Intent,
    fakes::{FakeProvider, account, app, flat_subjects, threaded},
};

/// A message on `thread`, in `mailbox`, delivered on `day` of June 2026.
fn dated_on_thread(id: &str, mailbox: &str, subject: &str, thread: &str, day: u8) -> Message {
    let mut message = threaded(id, mailbox, subject, thread);
    message.received_at = Some(format!("2026-06-{day:02}T09:00:00Z").parse().unwrap());
    message
}

/// The conversation rows' subjects and message counts, in list order; a search that groups
/// nothing leaves this empty and puts everything in [`flat_subjects`].
fn thread_subjects(snapshot: &MailboxListSnapshot) -> Vec<(String, u32)> {
    snapshot
        .rows
        .iter()
        .filter_map(|row| match row {
            SnapshotRow::Thread(thread) => Some((thread.subject.clone(), thread.message_count)),
            SnapshotRow::Flat(_) => None,
        })
        .collect()
}

/// Search obeys the list's grouping like every other list: with conversations on, matches are
/// shown as the conversations they belong to, not as loose messages.
#[tokio::test]
async fn a_threaded_search_groups_its_matches_into_conversations() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![
                dated_on_thread("m1", "a", "Report on the merger", "t1", 1),
                dated_on_thread("m2", "a", "Re: Report on the merger", "t1", 2),
                dated_on_thread("m3", "a", "Report on the roof", "t2", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    let list = app.mailbox_list();
    assert_eq!(
        thread_subjects(&list),
        [("Re: Report on the merger".to_owned(), 2)],
        "the two messages of one conversation are one row, summarised by the latest",
    );
    assert_eq!(
        flat_subjects(&list),
        ["Report on the roof"],
        "a match that is a conversation of one stays a flat row, as it does in the mailbox",
    );
    assert_eq!(
        list.total,
        list.rows.len(),
        "search results have no 'show more', so the total is what is on screen",
    );
}

/// A conversation row in a search stands for its **whole** thread, the messages that did not
/// match included: it is the same row the mailbox list draws, so selecting it and archiving it
/// means the same thing here as everywhere else (`docs/list-selection.md`).
///
/// What it is *labelled* with is the newest message that matched, not the newest message it
/// holds. Both fall out of one rule (a thread is summarised by its latest in-scope member, and
/// in a search "in scope" means "matched"), and the label is the half a person sees: a row
/// answering a search for "Report" has to say `Report`, or the result reads as the wrong answer.
#[tokio::test]
async fn a_threaded_search_carries_the_messages_that_did_not_match() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![
                dated_on_thread("m1", "a", "Report on the merger", "t1", 1),
                // Same conversation, and no query would match it on its own.
                dated_on_thread("m2", "a", "Re: lunch on Thursday", "t1", 2),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        thread_subjects(&app.mailbox_list()),
        [("Report on the merger".to_owned(), 2)],
        "the conversation is carried whole (both messages) and labelled by the one that matched",
    );
}

/// Newest-first (rule 1) is measured against the **query**: a conversation sorts on its newest
/// matching message, so a reply about something else does not carry an old thread to the top.
#[tokio::test]
async fn a_threaded_search_orders_a_conversation_by_its_newest_match() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![
                // The oldest match, on a thread whose newest message matches nothing.
                dated_on_thread("m1", "a", "Report on the merger", "t1", 1),
                dated_on_thread("m2", "a", "Re: lunch on Thursday", "t1", 4),
                // A newer match, on its own thread.
                dated_on_thread("m3", "a", "Report on the roof", "t2", 2),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    let list = app.mailbox_list();
    let order: Vec<String> = list
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Thread(thread) => thread.subject.clone(),
            SnapshotRow::Flat(flat) => flat.subject.clone(),
        })
        .collect();
    assert_eq!(
        order,
        ["Report on the roof", "Report on the merger"],
        "the conversation whose match is older sorts below, even though its latest message \
         (the Thursday reply, day 4) is the newest thing in the list",
    );
}
