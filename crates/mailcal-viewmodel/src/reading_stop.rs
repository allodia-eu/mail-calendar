//! Where the reading pane goes when the message in it leaves the list
//! (`docs/reading-actions.md`, "Where the pane goes next"), decided once for every client.
//!
//! The pane lands where a click would have taken the reader: the next message down, else the one
//! above, opened exactly as clicking its row opens it. So a collapsed conversation is its newest
//! message, and a draft says it is one, so it resumes in a composer rather than opening for
//! reading.

use crate::{
    row_menu::MenuMessage,
    view::{FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage, ThreadRow},
};

/// The message clicking collapsed conversation `thread` opens: its newest, whatever its kind
/// (`docs/row-actions.md`, rule 2). In Drafts it is the draft the row summarises
/// (`docs/drafts.md`), which is what that list is of.
#[must_use]
pub fn conversation_opens(thread: &ThreadRow, showing_drafts: bool) -> Option<&ThreadMessage> {
    let summarised = showing_drafts
        .then(|| thread.messages.iter().find(|m| m.key == thread.latest_key))
        .flatten();
    summarised.or_else(|| thread.messages.first())
}

/// The message the reading pane opens once `account`'s `key` has left the list: the next one
/// down, else the one above, else none.
///
/// `expanded` says which conversations the list shows expanded, which is the client's to know: an
/// expanded conversation offers each of its messages, a collapsed one the message clicking it
/// opens. Asked **before** the archive or delete is dispatched, while `key` is still listed; a
/// message the list no longer shows answers none, and the pane empties.
#[must_use]
pub fn stop_after_removing(
    snapshot: &MailboxListSnapshot,
    account: &str,
    key: &str,
    expanded: impl Fn(&ThreadRow) -> bool,
) -> Option<MenuMessage> {
    let stops = stops(snapshot, &expanded);
    let at = stops
        .iter()
        .position(|stop| stop.account == account && stop.key == key)?;
    stops
        .get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|before| stops.get(before)))
        .cloned()
}

/// Every message the list offers to read, in the order it shows them.
fn stops(
    snapshot: &MailboxListSnapshot,
    expanded: &impl Fn(&ThreadRow) -> bool,
) -> Vec<MenuMessage> {
    snapshot
        .rows
        .iter()
        .flat_map(|row| match row {
            SnapshotRow::Flat(flat) => vec![of_flat(flat)],
            SnapshotRow::Thread(thread) if expanded(thread) => {
                thread.messages.iter().map(of_member).collect()
            }
            SnapshotRow::Thread(thread) => conversation_opens(thread, snapshot.showing_drafts)
                .map(of_member)
                .into_iter()
                .collect(),
        })
        .collect()
}

fn of_flat(row: &FlatRow) -> MenuMessage {
    MenuMessage {
        account: row.account.clone(),
        key: row.key.clone(),
        kind: row.kind,
    }
}

fn of_member(message: &ThreadMessage) -> MenuMessage {
    MenuMessage {
        account: message.account.clone(),
        key: message.key.clone(),
        kind: message.kind,
    }
}

#[cfg(test)]
#[path = "reading_stop_tests.rs"]
mod tests;
