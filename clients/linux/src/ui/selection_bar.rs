//! The bar over the message list while rows are selected: the count, and the six actions
//! `docs/list-selection.md` decides on.
//!
//! A revealer rather than a widget that comes and goes, so the list's top edge moves once, in an
//! animation, instead of jumping as the selection empties and fills.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::BulkAction;

use super::{AppInput, selection::SelectionSummary};
use crate::l10n;

pub(crate) struct SelectionBar {
    root: gtk::Revealer,
    count: gtk::Label,
    /// The read and flag buttons, whose label and action come from what is selected rather than
    /// from a fixed pair (`docs/list-selection.md`, rule 5).
    read: PairedButton,
    flag: PairedButton,
}

/// One of the two buttons that stands for a pair of actions, holding the action its current
/// label means so a click dispatches what the user read.
struct PairedButton {
    button: gtk::Button,
    action: Rc<Cell<BulkAction>>,
}

impl PairedButton {
    fn new(sender: &relm4::Sender<AppInput>, initial: BulkAction) -> Self {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        let paired = Self {
            button,
            action: Rc::new(Cell::new(initial)),
        };
        paired.set(initial);
        let input = sender.clone();
        let action = Rc::clone(&paired.action);
        paired
            .button
            .connect_clicked(move |_| input.emit(AppInput::ActOnSelection(action.get())));
        paired
    }

    /// Re-labels the button for `action` and makes a click run it.
    fn set(&self, action: BulkAction) {
        self.action.set(action);
        let label = action_label(action);
        self.button.set_label(label);
        self.button
            .update_property(&[AccessibleProperty::Label(label)]);
    }
}

impl SelectionBar {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let count = gtk::Label::new(None);
        count.set_xalign(0.0);
        count.set_hexpand(true);
        count.set_ellipsize(gtk::pango::EllipsizeMode::End);
        count.add_css_class("heading");

        let read = PairedButton::new(sender, BulkAction::MarkRead);
        let flag = PairedButton::new(sender, BulkAction::Flag);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.append(&read.button);
        actions.append(&flag.button);
        actions.append(&action_button(sender, BulkAction::Archive, false));
        actions.append(&action_button(sender, BulkAction::Delete, false));
        actions.append(&action_button(sender, BulkAction::PermanentlyDelete, true));

        let select_all = gtk::Button::with_label(l10n::action_select_all());
        select_all.add_css_class("flat");
        let input = sender.clone();
        select_all.connect_clicked(move |_| input.emit(AppInput::SelectAllRows));
        let clear = gtk::Button::from_icon_name("window-close-symbolic");
        clear.add_css_class("flat");
        clear.set_tooltip_text(Some(l10n::action_clear_selection()));
        clear.update_property(&[AccessibleProperty::Label(l10n::action_clear_selection())]);
        let input = sender.clone();
        clear.connect_clicked(move |_| input.emit(AppInput::ClearSelection));

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.append(&count);
        row.append(&actions);
        row.append(&select_all);
        row.append(&clear);
        // Horizontal scrolling rather than a squeeze: the pane can be dragged to 260 px, and a
        // row of buttons that vanishes at that width takes the delete the user was reaching for.
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroll.set_child(Some(&row));

        let root = gtk::Revealer::new();
        root.set_child(Some(&scroll));
        Self {
            root,
            count,
            read,
            flag,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Revealer {
        &self.root
    }

    /// Brings the bar to what is selected: hidden at nothing, else the count and the two paired
    /// actions the selection calls for.
    pub(crate) fn render(&self, summary: SelectionSummary) {
        self.root.set_reveal_child(summary.count > 0);
        if summary.count == 0 {
            return;
        }
        let count = l10n::selection_count(i64::try_from(summary.count).unwrap_or(i64::MAX));
        self.count.set_text(&count);
        self.read.set(summary.read_action());
        self.flag.set(summary.flag_action());
    }
}

/// A button whose action never changes with the selection.
fn action_button(
    sender: &relm4::Sender<AppInput>,
    action: BulkAction,
    destructive: bool,
) -> gtk::Button {
    let button = gtk::Button::with_label(action_label(action));
    button.add_css_class("flat");
    if destructive {
        button.add_css_class("destructive-action");
    }
    let input = sender.clone();
    button.connect_clicked(move |_| input.emit(AppInput::ActOnSelection(action)));
    button
}

fn action_label(action: BulkAction) -> &'static str {
    match action {
        BulkAction::MarkRead => l10n::action_mark_read(),
        BulkAction::MarkUnread => l10n::action_mark_unread(),
        BulkAction::Flag => l10n::action_flag(),
        BulkAction::Unflag => l10n::action_unflag(),
        BulkAction::Archive => l10n::action_archive(),
        BulkAction::Delete => l10n::action_move_to_trash(),
        BulkAction::PermanentlyDelete => l10n::action_delete_permanently(),
    }
}
