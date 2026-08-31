//! Tests for [`super`]'s mailbox-list projection: flat/threaded ordering, the
//! cross-account search merge, folder filtering, and account tagging. Split out of
//! `view.rs` (and grouped into behaviour-focused submodules) to keep every file under the
//! 500-line limit. The shared test fixtures live here; the `#[test]`s live in the submodules.

use std::sync::Arc;

use engine_api::{MailListRow, MailboxRole, Message, ThreadRef};
use engine_core::{
    ids::{AccountId, MailboxId, MessageId, ThreadId},
    membership::Memberships,
};

use super::*;
use crate::view_rows::flat_row;

#[path = "view_tests/flat.rs"]
mod flat;
#[path = "view_tests/folders_and_accounts.rs"]
mod folders_and_accounts;
#[path = "view_tests/search.rs"]
mod search;
#[path = "view_tests/threaded.rs"]
mod threaded;

/// A window large enough to return every row; most tests assert over the full projection,
/// so they pass `ALL`; the pagination tests pass a small limit on purpose.
const ALL: usize = usize::MAX;

fn message(id: &str, subject: &str, minute: u8, thread: Option<&str>) -> Message {
    let mut message = Message::new(
        MessageId::try_from(id).unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    message.envelope.subject = Some(subject.to_owned());
    message.received_at = Some(format!("2026-06-01T09:{minute:02}:00Z").parse().unwrap());
    message.thread = thread.map(|t| ThreadRef::provider_assigned(ThreadId::try_from(t).unwrap()));
    message
}

/// Pairs a message with an account id for the projection; in scope (as the app marks a
/// folder's own messages) and inbound by default; the cross-folder tests tweak both.
///
/// The fixtures are whole [`Message`]s put through the engine's own projection, so what the
/// view-model is handed here is exactly the row a store read returns.
fn at(account: &str, message: &Message) -> AccountMessage {
    AccountMessage {
        row: Arc::new(row(account, message)),
        in_scope: true,
        outgoing: false,
    }
}

/// An out-of-scope member (in the mailbox but not the viewed folder; e.g. a Sent reply seen
/// from the Inbox); `outgoing` marks it as the owner's own message.
fn member(account: &str, message: &Message, outgoing: bool) -> AccountMessage {
    AccountMessage {
        row: Arc::new(row(account, message)),
        in_scope: false,
        outgoing,
    }
}

fn row(account: &str, message: &Message) -> MailListRow {
    MailListRow::project(&AccountId::try_from(account).unwrap(), message)
}

/// Sets the RFC `Message-ID` header on a message, so cross-folder copies can be deduped.
fn with_message_id(mut message: Message, id: &str) -> Message {
    message.envelope.message_id = vec![engine_api::MessageIdHeader::new(id).unwrap()];
    message
}

/// Sets the `From` address, optionally with a display name: so the sender-projection tests
/// can distinguish "has a name" from "email only".
fn with_from(mut message: Message, name: Option<&str>, email: &str) -> Message {
    message.envelope.from = vec![match name {
        Some(name) => engine_api::EmailAddress::named(name, email),
        None => engine_api::EmailAddress::new(email),
    }];
    message
}

/// The conversation messages of the first (newest) thread row.
fn thread_messages(snapshot: &MailboxListSnapshot) -> Vec<ThreadMessage> {
    let SnapshotRow::Thread(thread) = &snapshot.rows[0] else {
        panic!("expected a thread row");
    };
    thread.messages.clone()
}

fn flat_subjects(snapshot: &MailboxListSnapshot) -> Vec<&str> {
    snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.subject.as_str(),
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect()
}

/// Builds a role-tagged mailbox for the folder-ordering test.
fn roled(key: &str, name: &str, role: MailboxRole) -> Mailbox {
    let mut mailbox = Mailbox::new(MailboxId::try_from(key).unwrap(), name);
    mailbox.role = Some(role);
    mailbox
}

/// A role-tagged mailbox the server has counted.
fn counted(key: &str, name: &str, role: MailboxRole, unread: u32) -> Mailbox {
    let mut mailbox = roled(key, name, role);
    mailbox.unread_count = Some(unread);
    mailbox
}

/// An account switcher row, expanded; what an account nobody has shut projects to.
fn account(id: &str, email: &str) -> AccountRow {
    AccountRow {
        id: id.to_owned(),
        email: email.to_owned(),
        expanded: true,
    }
}
