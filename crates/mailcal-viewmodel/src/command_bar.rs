//! The mail command bar, decided once for every client (`docs/command-bar.md`).
//!
//! The bar is read off the row menu over the same rows, never decided beside it: which half of a
//! toggle is on offer, which items respond, which message an item is about and what Delete does in
//! Trash are all the menu's answer, so a right-click and the bar above it cannot disagree. This
//! file decides only what the bar adds: which actions it carries, in which order, and what stands
//! in the overflow menu whatever the width.

use crate::{
    row_menu::{MenuMessage, MenuRow, RowMenu, row_menu},
    row_reach::RowAction,
    view::MailboxListSnapshot,
};

/// What a bar item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// An action on the selected rows, exactly as the row menu offers it.
    Act(RowAction),
    /// Sync every account now. It acts on the mailbox rather than on what is selected, so it is
    /// never disabled.
    Sync,
}

/// One item of the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    /// What it does, and so its label and icon.
    pub command: Command,
    /// Whether it responds. A disabled item keeps its place, so the bar keeps its shape.
    pub enabled: bool,
    /// The one message it acts on, as a row menu item's target does; `None` acts on the rows.
    pub target: Option<MenuMessage>,
}

/// The bar, in order.
///
/// A client draws `sections` in order with a divider between them. When they do not fit, items
/// leave from the **end**, the last section's last item first, into an overflow menu at the
/// bar's end, in the order they had on the bar; `menu` follows them there, past a separator,
/// whatever the width. The overflow button is drawn only while the menu holds something.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandBar {
    /// The items on the bar, grouped; never an empty section.
    pub sections: Vec<Vec<CommandItem>>,
    /// The items that always stand in the overflow menu.
    pub menu: Vec<CommandItem>,
}

/// The bar over `rows`, the selection, against `snapshot`. With no row selected every action is
/// disabled and each toggle names its affirmative half, which is what a bar standing over
/// nothing says. `desktop` is the standing bar; anything else is a phone's or tablet's selection
/// mode, which syncs by pulling the list and has no room for a menu of extras.
#[must_use]
pub fn command_bar(snapshot: &MailboxListSnapshot, rows: &[MenuRow], desktop: bool) -> CommandBar {
    let menu = row_menu(snapshot, rows, desktop);
    let actions = [
        pick(&menu, &[RowAction::Delete, RowAction::PermanentlyDelete]),
        pick(&menu, &[RowAction::Archive]),
        pick(&menu, &[RowAction::MoveToFolder]),
        pick(&menu, &[RowAction::Flag, RowAction::Unflag]),
        pick(&menu, &[RowAction::MarkRead, RowAction::MarkUnread]),
    ]
    .to_vec();
    if !desktop {
        return CommandBar {
            sections: vec![actions],
            menu: Vec::new(),
        };
    }
    let sync = CommandItem {
        command: Command::Sync,
        enabled: true,
        target: None,
    };
    CommandBar {
        sections: vec![actions, vec![sync]],
        menu: vec![pick(
            &menu,
            &[RowAction::MarkAsSpam, RowAction::MarkAsNotSpam],
        )],
    }
}

/// The menu's item doing one of `actions` (for a toggle, the half on offer), or the first of
/// them disabled where the menu has none: no row selected, or a report over several rows, which
/// is not a batch (`docs/list-selection.md`, rule 12).
fn pick(menu: &RowMenu, actions: &[RowAction]) -> CommandItem {
    menu.sections
        .iter()
        .flatten()
        .find(|item| actions.contains(&item.action))
        .map_or(
            CommandItem {
                command: Command::Act(actions[0]),
                enabled: false,
                target: None,
            },
            |item| CommandItem {
                command: Command::Act(item.action),
                enabled: item.enabled,
                target: item.target.clone(),
            },
        )
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
mod tests;
