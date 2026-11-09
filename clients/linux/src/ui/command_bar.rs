//! The mail command bar (`docs/command-bar.md`): one full-width row under the window's header
//! bar, New Mail at its leading edge over the folder pane and the core's actions from the message
//! list's leading edge onward.
//!
//! What the bar holds is the core's `command_bar` over the selection; this file gives each item
//! its words and glyph and says what picking it does, which is what the same item in a row's menu
//! does. It decides nothing about either.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gio};
use mailcal_bindings::{Command, CommandBar, CommandItem, RowAction, SelectedRow};

use super::{
    AppInput,
    command_overflow::{OverflowEntry, OverflowExtra, OverflowRow},
    folder_dialogs,
    message_move::{self, MoveContext},
    row_menu,
};
use crate::{l10n, ui::icons};

/// The prefix the bar's actions are inserted under, for its ⋯ menu.
const GROUP: &str = "command-bar";

/// One item, on the row or in its menu.
struct Slot {
    button: gtk::Button,
    content: adw::ButtonContent,
    action: gio::SimpleAction,
    item: Rc<RefCell<CommandItem>>,
}

pub(crate) struct CommandBarRow {
    root: gtk::Box,
    #[cfg(test)]
    compose: gtk::Button,
    lead: gtk::Box,
    actions: OverflowRow,
    group: gio::SimpleActionGroup,
    slots: RefCell<Vec<Slot>>,
    extras: RefCell<Vec<Slot>>,
    /// The section lengths and the menu's length the slots were built for.
    shape: RefCell<Option<(Vec<usize>, usize)>>,
    moves: Rc<RefCell<MoveContext>>,
    sender: relm4::Sender<AppInput>,
}

impl CommandBarRow {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let content = adw::ButtonContent::new();
        content.set_icon_name(icons::NEW_MESSAGE);
        content.set_label(l10n::action_compose());
        let compose = gtk::Button::new();
        compose.set_child(Some(&content));
        // The window's one primary button: it is the one control here that names no selection.
        compose.add_css_class("suggested-action");
        compose.update_property(&[AccessibleProperty::Label(l10n::action_compose())]);
        compose.set_valign(gtk::Align::Center);
        compose.set_margin_start(8);
        compose.set_margin_end(8);
        let input = sender.clone();
        compose.connect_clicked(move |_| input.emit(AppInput::BeginNew));
        // As wide as the folder pane, so the actions start where the list does; never narrower
        // than New Mail itself, so a pane dragged shut leaves it in place and the actions follow.
        let lead = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        lead.append(&compose);

        let actions = OverflowRow::new();
        actions.widget().set_margin_start(4);
        actions.widget().set_margin_end(4);
        let group = gio::SimpleActionGroup::new();
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .accessible_role(gtk::AccessibleRole::Toolbar)
            .build();
        root.set_margin_top(4);
        root.set_margin_bottom(4);
        root.append(&lead);
        root.append(actions.widget());
        root.insert_action_group(GROUP, Some(&group));
        Self {
            root,
            #[cfg(test)]
            compose,
            lead,
            actions,
            group,
            slots: RefCell::default(),
            extras: RefCell::default(),
            shape: RefCell::default(),
            moves: Rc::default(),
            sender: sender.clone(),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Keeps New Mail's slot as wide as `pane`'s first child, the folder pane, as the user drags
    /// it.
    pub(crate) fn follow(&self, pane: &gtk::Paned) {
        self.lead.set_size_request(pane.position(), -1);
        let lead = self.lead.clone();
        pane.connect_position_notify(move |pane| lead.set_size_request(pane.position(), -1));
    }

    /// Draws `bar`, the core's answer over the selection, with `moves` saying where Move can send
    /// it. A bar of the same shape is updated in place, so a focused item keeps its focus.
    pub(crate) fn render(&self, bar: &CommandBar, moves: MoveContext) {
        self.moves.replace(moves);
        let shape = (
            bar.sections
                .iter()
                .map(|section| section.items.len())
                .collect(),
            bar.menu.len(),
        );
        if self.shape.borrow().as_ref() != Some(&shape) {
            self.rebuild(bar);
            self.shape.replace(Some(shape));
        }
        let items = bar.sections.iter().flat_map(|section| &section.items);
        for (index, (slot, item)) in self.slots.borrow().iter().zip(items).enumerate() {
            self.update(slot, item);
            self.actions
                .relabel(index, row_menu::bar_label(item.command));
        }
        for (slot, item) in self.extras.borrow().iter().zip(&bar.menu) {
            self.update(slot, item);
        }
        self.actions.refit();
    }

    fn rebuild(&self, bar: &CommandBar) {
        for name in self.group.list_actions() {
            self.group.remove_action(&name);
        }
        let mut entries = Vec::new();
        let mut slots = Vec::new();
        for (section, items) in bar.sections.iter().enumerate() {
            for item in &items.items {
                let name = format!("item-{}", slots.len());
                let slot = self.slot(item, &name);
                entries.push(OverflowEntry {
                    widget: slot.button.clone().upcast(),
                    section,
                    label: row_menu::bar_label(item.command).to_owned(),
                    action: format!("{GROUP}.{name}"),
                });
                slots.push(slot);
            }
        }
        let mut extras = Vec::new();
        let mut extra_entries = Vec::new();
        for item in &bar.menu {
            let name = format!("extra-{}", extras.len());
            let slot = self.slot(item, &name);
            extra_entries.push(OverflowExtra {
                label: row_menu::bar_label(item.command).to_owned(),
                action: format!("{GROUP}.{name}"),
            });
            extras.push(slot);
        }
        self.actions.set_entries(entries, extra_entries);
        self.slots.replace(slots);
        self.extras.replace(extras);
    }

    /// A button and the menu action standing for `item`, both doing what picking it does.
    fn slot(&self, item: &CommandItem, name: &str) -> Slot {
        let content = adw::ButtonContent::new();
        let button = gtk::Button::new();
        button.set_child(Some(&content));
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        let item = Rc::new(RefCell::new(item.clone()));
        let action = gio::SimpleAction::new(name, None);
        let (picked, moves, input, anchor) = (
            Rc::clone(&item),
            Rc::clone(&self.moves),
            self.sender.clone(),
            button.clone(),
        );
        let run = Rc::new(move || run_item(&picked.borrow(), &moves.borrow(), &anchor, &input));
        let on_click = Rc::clone(&run);
        button.connect_clicked(move |_| on_click());
        action.connect_activate(move |_, _| run());
        self.group.add_action(&action);
        Slot {
            button,
            content,
            action,
            item,
        }
    }

    fn update(&self, slot: &Slot, item: &CommandItem) {
        slot.item.replace(item.clone());
        let label = row_menu::bar_label(item.command);
        slot.content.set_label(label);
        slot.content.set_icon_name(row_menu::bar_icon(item.command));
        slot.button
            .update_property(&[AccessibleProperty::Label(label)]);
        let enabled = item.enabled && (!is_move(item) || move_plan(&self.moves.borrow()).is_some());
        slot.button.set_sensitive(enabled);
        slot.action.set_enabled(enabled);
        if item.confirm {
            slot.button.add_css_class("destructive-action");
        } else {
            slot.button.remove_css_class("destructive-action");
        }
    }

    /// The labels on the row, in order, standing or moved.
    #[cfg(test)]
    pub(crate) fn labels(&self) -> Vec<String> {
        self.slots
            .borrow()
            .iter()
            .map(|slot| slot.content.label().to_string())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn button(&self, index: usize) -> gtk::Button {
        self.slots.borrow()[index].button.clone()
    }

    #[cfg(test)]
    pub(crate) fn compose(&self) -> &gtk::Button {
        &self.compose
    }

    #[cfg(test)]
    pub(crate) fn lead(&self) -> &gtk::Box {
        &self.lead
    }

    #[cfg(test)]
    pub(crate) fn overflow(&self) -> &OverflowRow {
        &self.actions
    }

    #[cfg(test)]
    pub(crate) fn action_enabled(&self, name: &str) -> bool {
        self.group
            .lookup_action(name)
            .is_some_and(|action| action.is_enabled())
    }
}

fn is_move(item: &CommandItem) -> bool {
    item.command
        == (Command::Act {
            action: RowAction::MoveToFolder,
        })
}

/// Where Move can send the selection: the row menu's own picker over it, or `None` while there
/// is nothing selected or nowhere to go.
fn move_plan(moves: &MoveContext) -> Option<(SelectedRow, message_move::MessageMove)> {
    let row = moves.selected().first()?.clone();
    let plan = message_move::plan(moves, &row)?;
    Some((row, plan))
}

fn run_item(
    item: &CommandItem,
    moves: &MoveContext,
    anchor: &gtk::Button,
    input: &relm4::Sender<AppInput>,
) {
    if !item.enabled {
        return;
    }
    match item.command {
        Command::Sync => input.emit(AppInput::RefreshRequested),
        // The picker the row menu's Move to folder… opens, over the selection.
        _ if is_move(item) => {
            if let Some((row, plan)) = move_plan(moves) {
                folder_dialogs::message_move_dialog(anchor, row, plan, input);
            }
        }
        Command::Act { .. } => input.emit(AppInput::BarPick(Box::new(item.clone()))),
    }
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
pub(super) mod tests;
