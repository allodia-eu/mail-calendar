//! Bringing the message `ListBox` to a new snapshot **in place**, rather than rebuilding it.
//!
//! The list used to be cleared and re-appended on every change. A sync commits mail in chunks,
//! so during a first sync that was every row's widget torn down and rebuilt several times a
//! second; for a change that usually touches one row. The other three clients do not do this:
//! Windows reconciles explicitly, and SwiftUI and Compose diff by a stable row key.
//!
//! What survives a reconcile is what the widget owns and the snapshot does not: an
//! `ExpanderRow`'s disclosure animation, the focus ring, and a row mid-press. That is also why
//! expansion is deliberately absent from [`MailboxRendering`](super::mailbox::MailboxRendering)
//! ; a rebuild under an animating expander replaces the row it is animating.

use std::collections::HashSet;

use gtk::prelude::ListBoxRowExt;

/// One row's identity and its rendered form, as the reconcile needs them.
pub(super) trait Row {
    /// What makes this the *same* row across two snapshots.
    fn key(&self) -> &str;
}

/// Brings `list` from `previous` to `next`, building a widget only for a row that is new or
/// whose rendering changed.
///
/// `build` is called with the index into `next`, because a row's widget depends on more than the
/// row (a conversation needs its expansion state and the input sender).
///
/// Rows are matched by [`Row::key`] and compared by `PartialEq`, so a row that moved keeps its
/// widget; the reorder is a reparent, not a rebuild.
pub(super) fn reconcile<T, F>(list: &gtk::ListBox, previous: &[T], next: &[T], build: F)
where
    T: Row + PartialEq,
    F: Fn(usize) -> gtk::Widget,
{
    // Removing a selected GTK row clears the selection, even when its replacement has the same
    // message identity. Carry that identity across the reconcile.
    let selected = list
        .selected_row()
        .and_then(|row| usize::try_from(row.index()).ok())
        .and_then(|index| previous.get(index))
        .map(|row| row.key().to_owned());
    let mut on_screen: Vec<&str> = previous.iter().map(Row::key).collect();
    let wanted: HashSet<&str> = next.iter().map(Row::key).collect();

    // Back to front, so an index stays valid while the ones behind it are still being read.
    for index in (0..on_screen.len()).rev() {
        if !wanted.contains(on_screen[index]) {
            remove_at(list, index);
            on_screen.remove(index);
        }
    }

    for (position, row) in next.iter().enumerate() {
        if on_screen.get(position) == Some(&row.key()) {
            // Already here. Rebuild it only if what it renders changed; matching by key alone
            // would leave a read message showing as unread.
            if previous.iter().find(|old| old.key() == row.key()) != Some(row) {
                remove_at(list, position);
                insert_at(list, position, &build(position));
            }
            continue;
        }
        let found = on_screen[position.min(on_screen.len())..]
            .iter()
            .position(|key| *key == row.key());
        if let Some(offset) = found {
            // A row that moved: reparent the widget it already has rather than building another.
            let from = position + offset;
            if let Some(child) = list.row_at_index(index_of(from)) {
                list.remove(&child);
                list.insert(&child, index_of(position));
            }
            let key = on_screen.remove(from);
            on_screen.insert(position, key);
        } else {
            insert_at(list, position, &build(position));
            on_screen.insert(position, row.key());
        }
    }

    if let Some(position) = selected.and_then(|key| next.iter().position(|row| row.key() == key)) {
        list.select_row(list.row_at_index(index_of(position)).as_ref());
    }
}

fn remove_at(list: &gtk::ListBox, index: usize) {
    if let Some(child) = list.row_at_index(index_of(index)) {
        list.remove(&child);
    }
}

fn insert_at(list: &gtk::ListBox, index: usize, widget: &gtk::Widget) {
    list.insert(widget, index_of(index));
}

/// GTK indexes rows with a signed `i32`; a list longer than that cannot be scrolled to anyway,
/// so saturating is the honest conversion.
fn index_of(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}
