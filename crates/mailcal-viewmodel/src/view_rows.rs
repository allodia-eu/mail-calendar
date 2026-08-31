//! Row projection for the mailbox list: turning [`AccountMessage`]s into the flat and threaded
//! rows [`crate::view::build`] hands a host. Split out of `view.rs` (which keeps the snapshot
//! types and the public entry points) to stay under the 500-line limit.
//!
//! **Every ordering here is total.** A host reconciles the row list by row id and animates the
//! difference, so two projections of the same mailbox must produce the same row *sequence*;
//! ordering by date alone is not enough, because messages tie (same instant, or both undated) and
//! the surrounding sort is stable, leaving tied rows in whatever order the input happened to
//! arrive in. That order is not stable: threads are grouped through a [`HashMap`], and the app's
//! message cache and its store reload need not agree. A reshuffle turns into a list-wide move
//! storm; on WinUI every moved row's container is destroyed and re-created, blanking the whole
//! list. So each sort below breaks its ties on [`row_key`] (or, for threads, the equivalent
//! `(account, thread_id)`), mirroring `calendar::sort_key`.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use engine_api::{MailRow, UtcDateTime};

use crate::{
    avatar::{self, Avatar},
    view::{
        AccountMessage, FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage, ThreadRow,
        ViewMode,
    },
};

/// A message's tiebreaker: `(account, provider key)`; precisely the identity a host reconciles a
/// flat row by (`m:<account>:<key>`), and unique across the projection, so appending it to any
/// sort key makes that ordering total.
fn row_key(item: &AccountMessage) -> (&str, &str) {
    (item.account(), item.key())
}

/// A message's display/ordering instant.
///
/// The engine already resolved it to the delivery date falling back to the `Date`-header instant,
/// so a Sent reply (whose stored copy may carry only `sent_at`) still sorts, and can be picked as
/// a thread's latest, and opening a conversation lands on the newest message whether it was
/// received or sent. Ordering lives in the core.
fn instant(row: &MailRow) -> Option<UtcDateTime> {
    row.date_utc
}

/// The first sender's **display name**, falling back to their email address when the header
/// carried no name (or only whitespace), and to an empty string when there is no sender at
/// all. A mailbox list shows who a message is *from*: a person's name reads far better than
/// their address, and the reading view still carries the full `Name <email>` for detail.
fn sender(row: &MailRow) -> String {
    match row.from_name.as_deref() {
        Some(name) if !name.trim().is_empty() => name.to_owned(),
        _ => row.from_addr.clone().unwrap_or_default(),
    }
}

/// The sender's email address, which is what an avatar is *of*.
///
/// [`sender`] collapses name and address into one display string, and that string cannot key
/// a colour: two people share a name. Every row shape falls out of this one rule: a flat row
/// names its sender, a thread row the latest sender, a Sent row the sender too: so there is
/// no per-folder special case.
fn sender_address(row: &MailRow) -> String {
    row.from_addr.clone().unwrap_or_default()
}

/// The monogram and colour for a row's sender, with no photo yet.
///
/// Photos are filled in later by the app layer, which owns the cache: this projection runs
/// inside `rebuild_snapshot` and must not grow a store read, let alone a network fetch.
fn avatar_for(row: &MailRow) -> Avatar {
    avatar::resolve(&sender(row), &sender_address(row), None)
}

/// Formats a UTC instant as a whole-second `…Z` string for a host to localise. The
/// engine's `UtcDateTime::to_string` can carry sub-second precision (`…45.123Z`),
/// which the native RFC 3339 date parsers reject; they would then fall through to a
/// raw, un-localised wall-clock. Display granularity is minutes, so the sub-second
/// component is dropped here and every emitted instant parses reliably.
fn format_instant(instant: UtcDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        instant.year(),
        instant.month(),
        instant.day(),
        instant.hour(),
        instant.minute(),
        instant.second(),
    )
}

pub(crate) fn flat_row(item: &AccountMessage) -> FlatRow {
    let row = &item.row.mail;
    FlatRow {
        account: item.account().to_owned(),
        key: row.key.as_str().to_owned(),
        subject: row.subject.clone().unwrap_or_default(),
        from: sender(row),
        from_address: sender_address(row),
        avatar: avatar_for(row),
        // The same instant the row is ordered by (delivery date, else the `Date` header), so a
        // Sent copy carrying only `sent_at` shows a date instead of a blank; search lists them
        // beside received mail.
        date: instant(row).map_or_else(String::new, format_instant),
        unread: row.flags.is_unread(),
        flagged: row.flags.flagged(),
        has_attachment: row.has_attachment,
        preview: row.preview.clone().unwrap_or_default(),
    }
}

/// A flat list of the **in-scope** messages, newest received first. Builds a row only for the
/// first `limit` messages: the full ordering runs first, so the window is the true newest
/// `limit`; `total` carries the full in-scope count.
pub(crate) fn build_flat(messages: &[&AccountMessage], limit: usize) -> MailboxListSnapshot {
    let mut ordered: Vec<&AccountMessage> =
        messages.iter().copied().filter(|m| m.in_scope).collect();
    // Newest received first, ties broken on `row_key` so the window doesn't depend on the order
    // the caller supplied (the app's message cache and its store reload need not agree).
    ordered.sort_by(|a, b| {
        Reverse(instant(&a.row.mail))
            .cmp(&Reverse(instant(&b.row.mail)))
            .then_with(|| row_key(a).cmp(&row_key(b)))
    });
    MailboxListSnapshot {
        mode: ViewMode::Flat,
        total: ordered.len(),
        rows: ordered
            .into_iter()
            .take(limit)
            .map(flat_row)
            .map(SnapshotRow::Flat)
            .collect(),
        ..Default::default()
    }
}

/// A flat list of **search hits**, newest first: the same ordering the mailbox list uses, over
/// the messages the engine matched. Every hit is shown (a hit is in scope by definition, and
/// search never groups into conversations); `total` equals the returned rows, so the host asks
/// for no further page.
///
/// Ordering on time rather than relevance is the point: a person searching mail is looking for
/// *a* message and reads the list by when things arrived, so a relevance order, which
/// interleaves this morning's mail with a three-year-old thread; reads as no order at all. The
/// engine's ranking still decides *which* hits get here (it caps each account's query); this
/// decides the order they are shown in.
///
/// Unlike [`build_flat`] the key is [`instant`], not `received_at` alone: search reaches into
/// Sent, whose stored copies can carry only `sent_at`, and those must sort by when they were
/// sent instead of sinking below every dated message. Ties break on [`row_key`], the same total
/// order the rest of this module keeps.
pub(crate) fn build_search(hits: &[&AccountMessage], limit: usize) -> MailboxListSnapshot {
    let mut ordered: Vec<&AccountMessage> = hits.to_vec();
    ordered.sort_by(|a, b| {
        Reverse(instant(&a.row.mail))
            .cmp(&Reverse(instant(&b.row.mail)))
            .then_with(|| row_key(a).cmp(&row_key(b)))
    });
    let rows: Vec<SnapshotRow> = ordered
        .into_iter()
        .take(limit)
        .map(flat_row)
        .map(SnapshotRow::Flat)
        .collect();
    MailboxListSnapshot {
        mode: ViewMode::Flat,
        total: rows.len(),
        rows,
        ..Default::default()
    }
}

/// One grouped thread awaiting projection: its latest-activity instant (the sort key), its
/// `(account, thread_id)`, and its member messages; built for every thread, but turned into
/// a [`ThreadRow`] only for the windowed ones.
type GroupedThread<'a> = (
    Option<UtcDateTime>,
    (String, String),
    Vec<&'a AccountMessage>,
);

/// Conversations grouped by `(account, thread_id)` (an unthreaded message is its own
/// thread; the account is part of the key so two accounts' threads never merge), each
/// summarised by its latest message, newest activity first. Grouping sees **every** message
/// across folders, but only threads with an **in-scope** member are shown: so the Inbox
/// lists the conversations that touch it while each still carries its Sent replies. A
/// [`ThreadRow`] is built only for the first `limit` visible threads (the window); `total`
/// carries the full count of visible threads.
pub(crate) fn build_threaded(messages: &[&AccountMessage], limit: usize) -> MailboxListSnapshot {
    let mut groups: HashMap<(String, String), Vec<&AccountMessage>> = HashMap::new();
    for item in messages {
        let thread = item
            .row
            .mail
            .thread_id
            .as_ref()
            .map_or_else(|| item.key().to_owned(), |t| t.as_str().to_owned());
        groups
            .entry((item.account().to_owned(), thread))
            .or_default()
            .push(item);
    }

    // Keep only threads that touch the shown view (an in-scope member), then order by latest
    // activity using a cheap key, so only the windowed threads pay for a full row build below.
    let mut ordered: Vec<GroupedThread> = groups
        .into_iter()
        .filter(|(_, members)| members.iter().any(|item| item.in_scope))
        .map(|(key, members)| {
            // Order a thread by its latest IN-SCOPE message: the newest one that actually touches
            // the viewed folder. A follow-up the owner sends (filed in Sent, out of scope in the
            // Inbox) rides in the conversation for reference but must NOT jump the thread up the
            // list. The group is kept because a member is in scope, so this is always defined.
            let latest = members
                .iter()
                .filter(|item| item.in_scope)
                .map(|item| instant(&item.row.mail))
                .max()
                .unwrap_or(None);
            (latest, key, members)
        })
        .collect();
    // Newest first, ties broken on the unique `(account, thread_id)` key; `groups` is a `HashMap`,
    // whose iteration order is re-seeded on every rebuild (see this module's header).
    ordered
        .sort_by(|(a, ka, _), (b, kb, _)| Reverse(*a).cmp(&Reverse(*b)).then_with(|| ka.cmp(kb)));

    MailboxListSnapshot {
        mode: ViewMode::Threaded,
        total: ordered.len(),
        rows: ordered
            .into_iter()
            .take(limit)
            .map(|(_, (account, thread_id), members)| {
                // A conversation that collapses to a single message is not a thread: project it
                // as a flat row (no expand affordance, opens directly), so only real
                // multi-message conversations render as threads. This mirrors every mail client
                // and keeps every host free of the "is this really a thread?" check.
                let deduped = dedup_conversation(&members);
                if deduped.len() == 1 {
                    SnapshotRow::Flat(flat_row(deduped[0]))
                } else {
                    SnapshotRow::Thread(thread_row(account, thread_id, &deduped))
                }
            })
            .collect(),
        ..Default::default()
    }
}

/// Orders a conversation group's `members` newest-first and collapses cross-folder copies of one
/// message (same RFC `Message-ID`; e.g. a Gmail "All Mail" duplicate of an Inbox message) to a
/// single entry, keeping the in-scope copy so a row opens the message the user is looking at.
/// The deduped result drives both the flat-vs-thread decision (a conversation that collapses to
/// one message is projected as a flat row, not a one-message "thread") and [`thread_row`].
fn dedup_conversation<'a>(members: &[&'a AccountMessage]) -> Vec<&'a AccountMessage> {
    let mut ordered: Vec<&AccountMessage> = members.to_vec();
    // Newest first; among same-instant copies, the in-scope one leads so dedup keeps it. `row_key`
    // then settles what's left, so a conversation's expanded sub-rows (and which copy survives
    // dedup) don't depend on the order the group's members were collected in.
    ordered.sort_by(|a, b| {
        Reverse(instant(&a.row.mail))
            .cmp(&Reverse(instant(&b.row.mail)))
            .then_with(|| Reverse(a.in_scope).cmp(&Reverse(b.in_scope)))
            .then_with(|| row_key(a).cmp(&row_key(b)))
    });

    let mut seen: HashSet<&str> = HashSet::new();
    ordered
        .into_iter()
        .filter(|item| {
            // Dedup only on a present Message-ID (a threading hint, may be absent/duplicated);
            // a copy without one keeps its unique provider key, so it is never wrongly merged.
            item.row
                .mail
                .message_id
                .as_ref()
                .is_none_or(|id| seen.insert(id.as_str()))
        })
        .collect()
}

/// Summarises one conversation (its already-[`dedup_conversation`]ed `members`, newest-first,
/// never empty, and, by construction, always more than one: a single-message conversation is
/// projected as a flat row upstream) by its latest message, carrying the whole conversation so a
/// host can expand the row into a stacked reading view and open any message.
fn thread_row(account: String, thread_id: String, deduped: &[&AccountMessage]) -> ThreadRow {
    // The thread's representative for the viewed folder: its latest IN-SCOPE message (`deduped` is
    // newest-first, so the first in-scope member is the newest one that touches the folder). The
    // summary line and `latest_key` reflect it (not the newest message overall) so a Sent reply
    // filed elsewhere stays in `messages` for reference without becoming the thread's face in the
    // folder. Always defined: a shown thread has an in-scope member (fallback keeps it total).
    let representative = deduped
        .iter()
        .copied()
        .find(|item| item.in_scope)
        .unwrap_or(deduped[0]);
    let row = &representative.row.mail;
    ThreadRow {
        account,
        thread_id,
        latest_key: row.key.as_str().to_owned(),
        subject: row.subject.clone().unwrap_or_default(),
        latest_from: sender(row),
        latest_from_address: sender_address(row),
        avatar: avatar_for(row),
        latest_date: instant(row).map_or_else(String::new, format_instant),
        message_count: u32::try_from(deduped.len()).unwrap_or(u32::MAX),
        unread_count: u32::try_from(
            deduped
                .iter()
                .filter(|m| m.row.mail.flags.is_unread())
                .count(),
        )
        .unwrap_or(u32::MAX),
        has_attachment: deduped.iter().any(|m| m.row.mail.has_attachment),
        preview: row.preview.clone().unwrap_or_default(),
        messages: deduped.iter().map(|item| thread_message(item)).collect(),
    }
}

/// One conversation message row, from a member of the thread group.
fn thread_message(item: &AccountMessage) -> ThreadMessage {
    let row = &item.row.mail;
    ThreadMessage {
        account: item.account().to_owned(),
        key: row.key.as_str().to_owned(),
        from: sender(row),
        from_address: sender_address(row),
        avatar: avatar_for(row),
        date: instant(row).map_or_else(String::new, format_instant),
        preview: row.preview.clone().unwrap_or_default(),
        unread: row.flags.is_unread(),
        outgoing: item.outgoing,
        has_attachment: row.has_attachment,
    }
}
