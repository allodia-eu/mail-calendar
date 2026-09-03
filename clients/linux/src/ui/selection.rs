//! The message list's multi-selection: which rows are picked, and what the bar above them offers.
//!
//! Pure state over the snapshot's row order, so the rules (`docs/list-selection.md`) are testable
//! without a display. The widgets read it; they never hold a second copy.

use std::collections::HashSet;

use mailcal_bindings::{BulkAction, SelectedRow, SnapshotRow};

/// One selected row's identity: the account, plus a message key or a thread id.
///
/// Account-scoped because a provider key is unique only within its account, and the unified list
/// selects across accounts. Structured rather than the formatted string the reconciler matches
/// rows by, since that one has to be parsed back to act on it and an id may contain a slash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RowKey {
    account: String,
    id: String,
    /// Whether `id` names a conversation rather than a single message.
    thread: bool,
}

impl RowKey {
    pub(crate) fn of(row: &SnapshotRow) -> Self {
        match row {
            SnapshotRow::Flat { row } => Self {
                account: row.account.clone(),
                id: row.key.clone(),
                thread: false,
            },
            SnapshotRow::Thread { row } => Self {
                account: row.account.clone(),
                id: row.thread_id.clone(),
                thread: true,
            },
        }
    }

    fn selected_row(&self) -> SelectedRow {
        if self.thread {
            SelectedRow::Thread {
                account: self.account.clone(),
                thread_id: self.id.clone(),
            }
        } else {
            SelectedRow::Message {
                account: self.account.clone(),
                key: self.id.clone(),
            }
        }
    }
}

/// What a click on a row means, decided by the modifiers the user held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectMode {
    /// A plain click: this row alone, and it becomes the anchor.
    Replace,
    /// Ctrl-click: add or remove this row, and it becomes the anchor.
    Toggle,
    /// Shift-click: every row from the anchor to this one, replacing what was selected.
    Range,
}

/// Which of the paired actions the bar offers, decided by what is selected.
///
/// One button per pair, never both: a bar over a mixed selection has to choose, and the useful
/// choice is the one that changes something. Any unread row makes the button "Mark as read"; any
/// unflagged row makes it "Flag".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectionSummary {
    pub(crate) count: usize,
    /// Whether any selected row holds an unread message.
    pub(crate) any_unread: bool,
    /// Whether any selected row holds an unflagged message. A conversation has no flag of its
    /// own, so it counts as unflagged: flagging is what its rows can usefully be asked for.
    pub(crate) any_unflagged: bool,
}

impl SelectionSummary {
    /// The mark-read/mark-unread action the bar's single button runs.
    pub(crate) fn read_action(self) -> BulkAction {
        if self.any_unread {
            BulkAction::MarkRead
        } else {
            BulkAction::MarkUnread
        }
    }

    /// The flag/unflag action the bar's single button runs.
    pub(crate) fn flag_action(self) -> BulkAction {
        if self.any_unflagged {
            BulkAction::Flag
        } else {
            BulkAction::Unflag
        }
    }
}

/// The rows the user has picked, in list order, plus the anchor a range extends from.
#[derive(Debug, Default)]
pub(crate) struct Selection {
    rows: Vec<RowKey>,
    anchor: Option<RowKey>,
}

impl Selection {
    /// Applies a click on the row at `index` of `rows`.
    pub(crate) fn click(&mut self, rows: &[SnapshotRow], index: usize, mode: SelectMode) {
        let Some(clicked) = rows.get(index).map(RowKey::of) else {
            return;
        };
        match mode {
            SelectMode::Replace => {
                self.rows = vec![clicked.clone()];
                self.anchor = Some(clicked);
            }
            SelectMode::Toggle => {
                match self.rows.iter().position(|row| row == &clicked) {
                    Some(position) => {
                        self.rows.remove(position);
                    }
                    None => self.rows.push(clicked.clone()),
                }
                self.anchor = Some(clicked);
            }
            // No anchor to extend from (the first click of the session was a Shift-click) means
            // there is no range either; treat it as picking the one row, which is what the user
            // gets from every list that has nothing to extend.
            SelectMode::Range => {
                let anchor = self
                    .anchor
                    .as_ref()
                    .and_then(|anchor| rows.iter().position(|row| &RowKey::of(row) == anchor));
                let Some(anchor) = anchor else {
                    self.rows = vec![clicked.clone()];
                    self.anchor = Some(clicked);
                    return;
                };
                let (first, last) = (anchor.min(index), anchor.max(index));
                self.rows = rows[first..=last].iter().map(RowKey::of).collect();
            }
        }
    }

    /// Selects every row the list is showing, which is the loaded window rather than the whole
    /// folder (`docs/list-selection.md`, rule 10).
    pub(crate) fn select_all(&mut self, rows: &[SnapshotRow]) {
        self.rows = rows.iter().map(RowKey::of).collect();
        self.anchor = self.rows.first().cloned();
    }

    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.anchor = None;
    }

    /// Drops anything `rows` no longer holds: a message that was archived, a folder the user has
    /// left, a search that replaced the list. A selection outliving its list acts on rows nobody
    /// can see (rule 4).
    pub(crate) fn retain_listed(&mut self, rows: &[SnapshotRow]) {
        // A set, not a list: this runs on every snapshot the core publishes, and Select all over a
        // long window would otherwise compare every selected row against every listed one.
        let listed: HashSet<RowKey> = rows.iter().map(RowKey::of).collect();
        self.rows.retain(|row| listed.contains(row));
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !listed.contains(anchor))
        {
            self.anchor = None;
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn contains(&self, row: &SnapshotRow) -> bool {
        let key = RowKey::of(row);
        self.rows.iter().any(|selected| selected == &key)
    }

    /// The selected rows in the shape [`mailcal_bindings::Intent::ActOnSelection`] takes.
    pub(crate) fn selected_rows(&self) -> Vec<SelectedRow> {
        self.rows.iter().map(RowKey::selected_row).collect()
    }

    /// What the bar says and which of the paired actions it offers.
    pub(crate) fn summary(&self, rows: &[SnapshotRow]) -> SelectionSummary {
        let mut summary = SelectionSummary {
            count: self.rows.len(),
            any_unread: false,
            any_unflagged: false,
        };
        for row in rows.iter().filter(|row| self.contains(row)) {
            match row {
                SnapshotRow::Flat { row } => {
                    summary.any_unread |= row.unread;
                    summary.any_unflagged |= !row.flagged;
                }
                SnapshotRow::Thread { row } => {
                    summary.any_unread |= row.unread_count > 0;
                    summary.any_unflagged = true;
                }
            }
        }
        summary
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
