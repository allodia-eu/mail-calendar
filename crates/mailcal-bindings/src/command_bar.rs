//! The mail command bar (`docs/command-bar.md`): asked for whenever the selection changes, drawn
//! as answered.
//!
//! A client draws the sections in order with a divider between them, each item under its label
//! and icon, greyed out where it is not enabled. When the row runs out of width, items leave from
//! the end into an overflow menu, in bar order, and `menu` follows them there past a separator;
//! the overflow button is drawn only while that menu holds something. A pick runs exactly as the
//! same item in a row menu does: the single-message form for an item with a `target`, the batch
//! over the selection for one without, and a question first where `confirm` is set.

use mailcal_viewmodel::{
    Command as AppCommand, CommandBar as AppCommandBar, CommandItem as AppCommandItem,
};

use crate::{
    MailcalApp, MenuMessage, RowAction, SelectedRow,
    row_menu::{asks_first, row_ref},
};

/// What a bar item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Command {
    /// An action on the selection, as the row menu offers it; labelled and drawn as that action.
    Act {
        /// The action.
        action: RowAction,
    },
    /// Sync every account now. Never disabled.
    Sync,
}

/// One item of the bar.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CommandItem {
    /// What it does.
    pub command: Command,
    /// Whether it responds; a disabled item keeps its place.
    pub enabled: bool,
    /// The one message it acts on; `None` acts on the selection.
    pub target: Option<MenuMessage>,
    /// Whether a client asks before acting: set for what cannot be undone, and only for that.
    pub confirm: bool,
}

/// One group of the bar: the items between two dividers.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CommandSection {
    /// The items, in order; never empty.
    pub items: Vec<CommandItem>,
}

/// The bar.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct CommandBar {
    /// The groups on the bar, in order. Items overflow from the end of the last one.
    pub sections: Vec<CommandSection>,
    /// What always stands in the overflow menu, after whatever overflowed.
    pub menu: Vec<CommandItem>,
}

#[uniffi::export]
impl MailcalApp {
    /// The mail command bar over `rows`, the selection; empty selects nothing, and every action
    /// is then disabled. Answered from the current mailbox snapshot without waiting on anything,
    /// so a client asks whenever the selection or the snapshot changes.
    pub fn command_bar(&self, rows: Vec<SelectedRow>) -> CommandBar {
        let rows: Vec<_> = rows.into_iter().filter_map(row_ref).collect();
        self.app.command_bar(&rows).into()
    }
}

impl From<AppCommandItem> for CommandItem {
    fn from(item: AppCommandItem) -> Self {
        let (command, confirm) = match item.command {
            AppCommand::Act(action) => (
                Command::Act {
                    action: action.into(),
                },
                asks_first(action),
            ),
            AppCommand::Sync => (Command::Sync, false),
        };
        Self {
            command,
            enabled: item.enabled,
            target: item.target.map(MenuMessage::from),
            confirm,
        }
    }
}

impl From<AppCommandBar> for CommandBar {
    fn from(bar: AppCommandBar) -> Self {
        Self {
            sections: bar
                .sections
                .into_iter()
                .map(|items| CommandSection {
                    items: items.into_iter().map(CommandItem::from).collect(),
                })
                .collect(),
            menu: bar.menu.into_iter().map(CommandItem::from).collect(),
        }
    }
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
mod tests;
