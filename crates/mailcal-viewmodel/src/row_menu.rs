//! The menu a mailbox-list row opens, decided once for every client (`docs/row-actions.md`).
//!
//! A client asks about the rows it is opening a menu over (the one under the pointer, or the
//! selection that row belongs to) and draws what comes back: which items, in which order, in
//! which sections, which of them respond, and for an item about one message, which message that
//! is. It decides none of it itself, because a conversation row's answer depends on which of its
//! messages is the newest, which was received and where each is filed, and five clients working
//! that out would be five answers.

use crate::{
    row_reach::{AccountPlaces, Member, MessageKind, RowAction, conversation_reach, stays_put},
    view::{FlatRow, MailboxListSnapshot, SnapshotRow, ThreadMessage},
};

/// A row a menu is opened over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuRow {
    /// One message: a flat row, or one message of an expanded conversation.
    Message {
        /// The account holding it.
        account: String,
        /// Its provider key.
        key: String,
    },
    /// One conversation row, standing for every message on it.
    Thread {
        /// The account holding it.
        account: String,
        /// The thread's id, as the snapshot's row carries it.
        thread_id: String,
    },
}

/// The one message an item is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuMessage {
    /// The account holding it.
    pub account: String,
    /// Its provider key.
    pub key: String,
    /// What it is to its account, which decides how a client opens it: a draft resumes in a
    /// composer, anything else opens for reading.
    pub kind: MessageKind,
}

/// One item of a row menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowMenuItem {
    /// What the item does, and so its label.
    pub action: RowAction,
    /// Whether it responds. A disabled item is still drawn, where it always is, so the menu
    /// keeps its shape from one row to the next.
    pub enabled: bool,
    /// The one message the item acts on, when it acts on one message: a client dispatches the
    /// single-message form. `None` when it acts on the rows the menu was asked about: a client
    /// dispatches the batch over those rows, and the core works out the messages.
    pub target: Option<MenuMessage>,
}

/// A row menu: its items in order, grouped into the sections a client separates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowMenu {
    /// The sections, in order; never an empty one.
    pub sections: Vec<Vec<RowMenuItem>>,
}

/// The menu for `rows` against `snapshot`: one row's own menu, or, for several, the menu that
/// acts on all of them at once (`docs/list-selection.md`, rule 12). `desktop` adds the items
/// only a desktop offers.
///
/// A row the snapshot no longer lists is left out; with none left the menu is empty.
#[must_use]
pub fn row_menu(snapshot: &MailboxListSnapshot, rows: &[MenuRow], desktop: bool) -> RowMenu {
    let found: Vec<Found> = rows.iter().filter_map(|row| find(snapshot, row)).collect();
    let sections = match found.as_slice() {
        [] => Vec::new(),
        [row] => one_row(row, desktop),
        rows => many_rows(rows),
    };
    RowMenu {
        sections: sections.into_iter().filter(|s| !s.is_empty()).collect(),
    }
}

/// One message on a row, read off the snapshot.
struct Fact<'s> {
    account: &'s str,
    key: &'s str,
    kind: MessageKind,
    unread: bool,
    flagged: bool,
    folders: &'s [String],
}

impl<'s> Fact<'s> {
    fn of_flat(row: &'s FlatRow) -> Self {
        Self {
            account: &row.account,
            key: &row.key,
            kind: row.kind,
            unread: row.unread,
            flagged: row.flagged,
            folders: &row.folders,
        }
    }

    fn of_thread(message: &'s ThreadMessage) -> Self {
        Self {
            account: &message.account,
            key: &message.key,
            kind: message.kind,
            unread: message.unread,
            flagged: message.flagged,
            folders: &message.folders,
        }
    }

    fn member(&self) -> Member {
        Member {
            kind: self.kind,
            unread: self.unread,
            flagged: self.flagged,
        }
    }

    fn message(&self) -> MenuMessage {
        MenuMessage {
            account: self.account.to_owned(),
            key: self.key.to_owned(),
            kind: self.kind,
        }
    }

    fn filed_in(&self, folder: &str) -> bool {
        self.folders.iter().any(|key| key == folder)
    }
}

/// A row the menu is about: one message, or a conversation's messages, newest first.
struct Found<'s> {
    members: Vec<Fact<'s>>,
    conversation: bool,
    places: Option<&'s AccountPlaces>,
}

fn find<'s>(snapshot: &'s MailboxListSnapshot, row: &MenuRow) -> Option<Found<'s>> {
    let (account, members, conversation) = match row {
        MenuRow::Message { account, key } => {
            (account, vec![find_message(snapshot, account, key)?], false)
        }
        MenuRow::Thread { account, thread_id } => {
            let thread = snapshot.rows.iter().find_map(|row| match row {
                SnapshotRow::Thread(t) if &t.account == account && &t.thread_id == thread_id => {
                    Some(t)
                }
                _ => None,
            })?;
            let members = thread.messages.iter().map(Fact::of_thread).collect();
            (account, members, true)
        }
    };
    Some(Found {
        members,
        conversation,
        places: snapshot.places.iter().find(|p| &p.account == account),
    })
}

fn find_message<'s>(
    snapshot: &'s MailboxListSnapshot,
    account: &str,
    key: &str,
) -> Option<Fact<'s>> {
    snapshot.rows.iter().find_map(|row| match row {
        SnapshotRow::Flat(f) if f.account == account && f.key == key => Some(Fact::of_flat(f)),
        SnapshotRow::Thread(t) => t
            .messages
            .iter()
            .find(|m| m.account == account && m.key == key)
            .map(Fact::of_thread),
        SnapshotRow::Flat(_) => None,
    })
}

impl Found<'_> {
    /// The newest message: what Open, Reply and Forward act on, whatever its kind.
    fn latest(&self) -> &Fact<'_> {
        &self.members[0]
    }

    fn reach(&self, action: RowAction) -> Vec<&Fact<'_>> {
        if !self.conversation {
            return self.members.iter().collect();
        }
        let members: Vec<Member> = self.members.iter().map(Fact::member).collect();
        conversation_reach(action, &members)
            .into_iter()
            .map(|at| &self.members[at])
            .collect()
    }

    /// The read toggle this row offers: Mark read while anything it reaches is unread.
    fn read_action(&self) -> RowAction {
        if self.reach(RowAction::MarkRead).iter().any(|m| m.unread) {
            RowAction::MarkRead
        } else {
            RowAction::MarkUnread
        }
    }

    /// The flag toggle this row offers: Unflag while anything on it is flagged.
    fn flag_action(&self) -> RowAction {
        if self.members.iter().any(|m| m.flagged) {
            RowAction::Unflag
        } else {
            RowAction::Flag
        }
    }

    /// The messages a move to `destination` takes: a conversation's Sent copies and drafts
    /// stay ([`stays_put`]), and a message already there has nowhere to go.
    fn moving(&self, destination: Option<&str>) -> Vec<&Fact<'_>> {
        let kept = self.places.map_or(&[][..], |p| p.kept.as_slice());
        self.members
            .iter()
            .filter(|m| {
                !(self.conversation
                    && stays_put(m.kind, m.folders.iter().map(String::as_str), kept))
            })
            .filter(|m| destination.is_none_or(|folder| !m.filed_in(folder)))
            .collect()
    }

    /// Whether a move to the account's folder `place` (Archive, Trash) takes anything.
    fn moves_to(&self, place: impl Fn(&AccountPlaces) -> Option<&String>) -> bool {
        self.places
            .and_then(place)
            .is_some_and(|folder| !self.moving(Some(folder)).is_empty())
    }

    /// The one message a report is about: the row's own message, or a conversation's newest
    /// received one.
    fn report_target(&self) -> Option<&Fact<'_>> {
        if self.conversation {
            self.members
                .iter()
                .find(|m| m.kind == MessageKind::Received)
        } else {
            self.members.first()
        }
    }
}

/// The item for `action`, acting on `target` when it is about one message.
fn item(action: RowAction, enabled: bool, target: Option<&Fact<'_>>) -> RowMenuItem {
    RowMenuItem {
        action,
        enabled,
        target: target.map(Fact::message),
    }
}

/// One row's own menu.
fn one_row(row: &Found<'_>, desktop: bool) -> Vec<Vec<RowMenuItem>> {
    let latest = row.latest();
    // An item that writes acts on the row's message when the row *is* one message, and on the
    // rows otherwise, where the core works out the members at write time.
    let own = (!row.conversation).then_some(latest);

    let mut open = vec![item(RowAction::Open, true, Some(latest))];
    if desktop {
        open.push(item(RowAction::OpenInWindow, true, Some(latest)));
    }

    let answerable = latest.kind != MessageKind::Draft;
    let respond = [RowAction::Reply, RowAction::ReplyAll, RowAction::Forward]
        .map(|action| item(action, answerable, Some(latest)))
        .to_vec();

    let read = row.read_action();
    let flag = row.flag_action();
    let flag_target = match flag {
        RowAction::Flag if row.conversation => row.reach(flag).first().copied(),
        _ => own,
    };
    let state = vec![
        item(read, !row.reach(read).is_empty(), own),
        item(flag, !row.reach(flag).is_empty(), flag_target),
    ];

    let report = report_item(row);

    let filing = vec![
        item(RowAction::MoveToFolder, !row.moving(None).is_empty(), own),
        item(
            RowAction::Archive,
            row.moves_to(|p| p.archive.as_ref()),
            own,
        ),
        item(RowAction::Delete, row.moves_to(|p| p.trash.as_ref()), own),
        item(
            RowAction::PermanentlyDelete,
            !row.moving(None).is_empty(),
            own,
        ),
    ];
    vec![open, respond, state, vec![report], filing]
}

/// Report as junk, or as not junk once the message it is about is in Junk.
fn report_item(row: &Found<'_>) -> RowMenuItem {
    let target = row.report_target();
    let junk = row.places.and_then(|p| p.junk.as_deref());
    let in_junk = target.is_some_and(|m| junk.is_some_and(|folder| m.filed_in(folder)));
    if in_junk {
        let inbox = row.places.and_then(|p| p.inbox.as_ref());
        item(RowAction::MarkAsNotSpam, inbox.is_some(), target)
    } else {
        let received = target.is_some_and(|m| m.kind == MessageKind::Received);
        item(RowAction::MarkAsSpam, received && junk.is_some(), target)
    }
}

/// The menu over several rows: the batch actions, labelled as the selection bar labels them
/// (`docs/list-selection.md`, rule 5), acting on every row.
fn many_rows(rows: &[Found<'_>]) -> Vec<Vec<RowMenuItem>> {
    let any = |test: &dyn Fn(&Found<'_>) -> bool| rows.iter().any(test);
    let read = if any(&|row| row.read_action() == RowAction::MarkRead) {
        RowAction::MarkRead
    } else {
        RowAction::MarkUnread
    };
    let flag = if any(&|row| row.flag_action() == RowAction::Flag) {
        RowAction::Flag
    } else {
        RowAction::Unflag
    };
    let state = vec![
        item(read, any(&|row| !row.reach(read).is_empty()), None),
        item(flag, any(&|row| !row.reach(flag).is_empty()), None),
    ];

    let mut filing = Vec::new();
    // Moving to a named folder stays within one account (`docs/folder-pane.md`, rule 24).
    let account = rows[0].latest().account;
    if rows.iter().all(|row| row.latest().account == account) {
        filing.push(item(
            RowAction::MoveToFolder,
            any(&|row| !row.moving(None).is_empty()),
            None,
        ));
    }
    filing.extend([
        item(
            RowAction::Archive,
            any(&|row| row.moves_to(|p| p.archive.as_ref())),
            None,
        ),
        item(
            RowAction::Delete,
            any(&|row| row.moves_to(|p| p.trash.as_ref())),
            None,
        ),
        item(
            RowAction::PermanentlyDelete,
            any(&|row| !row.moving(None).is_empty()),
            None,
        ),
    ]);
    vec![state, filing]
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
