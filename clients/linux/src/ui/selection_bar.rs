//! The mail actions bar over the mailbox: the count, and the six actions
//! `docs/list-selection.md` decides on.
//!
//! Standing chrome, spanning the list and the reading pane, with the actions merely insensitive
//! while nothing is picked (rule 5). A bar that came and went with the selection moved the rows
//! under the pointer and the message being read on every first click, and inside the list pane
//! its buttons were something to scroll sideways for at any width the user actually drags to.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{BulkAction, ViewMode};

use super::{AppInput, selection::SelectionSummary};
use crate::l10n;

pub(crate) struct SelectionBar {
    root: gtk::WindowHandle,
    /// The read and flag buttons, whose label, icon and action come from what is selected rather
    /// than from a fixed pair (`docs/list-selection.md`, rule 5).
    read: PairedButton,
    flag: PairedButton,
    /// Everything that names the selection, so it goes insensitive with an empty one. Select all
    /// is deliberately absent: it is how a pointer starts a selection in the first place.
    needs_selection: Vec<gtk::Button>,
}

/// One of the two buttons that stands for a pair of actions, holding the action its current
/// label means so a click dispatches what the user read.
struct PairedButton {
    button: gtk::Button,
    content: adw::ButtonContent,
    action: Rc<Cell<BulkAction>>,
}

impl PairedButton {
    fn new(sender: &relm4::Sender<AppInput>, initial: BulkAction) -> Self {
        let content = adw::ButtonContent::new();
        let button = gtk::Button::new();
        button.set_child(Some(&content));
        button.add_css_class("flat");
        let paired = Self {
            button,
            content,
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
        self.content.set_label(label);
        self.content.set_icon_name(action_icon(action));
        self.button
            .update_property(&[AccessibleProperty::Label(label)]);
    }
}

impl SelectionBar {
    pub(crate) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        let read = PairedButton::new(sender, BulkAction::MarkRead);
        let flag = PairedButton::new(sender, BulkAction::Flag);
        let archive = action_button(sender, BulkAction::Archive, false);
        let trash = action_button(sender, BulkAction::Delete, false);
        let purge = action_button(sender, BulkAction::PermanentlyDelete, true);

        let select_all = labelled_button(SELECT_ALL_ICON, l10n::action_select_all());
        let input = sender.clone();
        select_all.connect_clicked(move |_| input.emit(AppInput::SelectAllRows));
        let clear = gtk::Button::from_icon_name(CLEAR_ICON);
        clear.add_css_class("flat");
        clear.set_tooltip_text(Some(l10n::action_clear_selection()));
        clear.update_property(&[AccessibleProperty::Label(l10n::action_clear_selection())]);
        let input = sender.clone();
        clear.connect_clicked(move |_| input.emit(AppInput::ClearSelection));

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        for button in [
            &read.button,
            &flag.button,
            &archive,
            &trash,
            &purge,
            &select_all,
            &clear,
        ] {
            actions.append(button);
        }
        // Horizontal scrolling rather than a squeeze: the window can be narrowed until the seven
        // buttons no longer fit, and a row of them that vanishes takes the delete the user was
        // reaching for with it.
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroll.set_hexpand(true);
        scroll.set_child(Some(&actions));

        // Leading, where a toolbar's buttons live and where Outlook puts the same six. The bar
        // carries no count: that is the reading pane's, over the rows it would otherwise be
        // describing from a distance (`docs/list-selection.md`, rule 10).
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        bar.add_css_class("toolbar");
        // Over the `toolbar` class's own padding: seven buttons shoulder to shoulder read as one
        // long control rather than as seven things to choose between.
        bar.set_margin_top(4);
        bar.set_margin_bottom(4);
        bar.set_margin_start(4);
        bar.set_margin_end(4);
        bar.append(&scroll);
        // The bar is the mail surface's top row, so the window's own controls belong on it. The
        // reading pane's header sits a row below, and a close button there is a close button short
        // of the corner a pointer is thrown at. Every other page carries them on its rightmost
        // header, which is that page's top row.
        bar.append(&gtk::WindowControls::new(gtk::PackType::End));

        // A row holding the close button is the window's caption row, so it drags and double-clicks
        // like one; the buttons and the scroller still claim their own presses first.
        let root = gtk::WindowHandle::new();
        root.set_child(Some(&bar));
        let needs_selection = vec![
            read.button.clone(),
            flag.button.clone(),
            archive,
            trash,
            purge,
            clear,
        ];
        Self {
            root,
            read,
            flag,
            needs_selection,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::WindowHandle {
        &self.root
    }

    /// Brings the bar to what is selected: the two paired actions the selection calls for, and
    /// whether the actions can be pressed at all.
    pub(crate) fn render(&self, summary: SelectionSummary) {
        let selected = summary.count > 0;
        for button in &self.needs_selection {
            button.set_sensitive(selected);
        }
        self.read.set(summary.read_action());
        self.flag.set(summary.flag_action());
    }
}

/// What the reading pane says while several rows are picked: the count, over whatever the pane was
/// holding.
///
/// An overlay rather than another page of the detail stack, so the reading pane keeps its state:
/// dropping back to one selected row uncovers the message with no re-fetch, and the composer's
/// own swap of that stack is left alone.
pub(crate) struct SelectionCountPane {
    root: gtk::Box,
    label: gtk::Label,
}

impl SelectionCountPane {
    pub(crate) fn new() -> Self {
        let icon = gtk::Image::from_icon_name("mail-unread-symbolic");
        icon.set_pixel_size(32);
        icon.add_css_class("dim-label");
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        label.set_justify(gtk::Justification::Center);
        label.add_css_class("title-3");
        label.add_css_class("dim-label");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_valign(gtk::Align::Center);
        root.set_halign(gtk::Align::Fill);
        root.set_vexpand(true);
        root.append(&icon);
        root.append(&label);
        // Opaque, or the message underneath shows through the words describing the selection.
        root.add_css_class("view");
        root.set_visible(false);
        Self { root, label }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Shows the count, or lets the pane go back to what it was holding.
    pub(crate) fn render(&self, summary: SelectionSummary, mode: &ViewMode) {
        let stated = summary.pane_label(mode);
        if let Some(text) = &stated {
            self.label.set_text(text);
        }
        self.root.set_visible(stated.is_some());
    }
}

/// Assembles the mailbox surface: the actions bar over the list|reading split.
///
/// The bar belongs to neither pane. It acts on rows, but its buttons are as wide as the words on
/// them, and a list pane the user can drag to 340 px made the last of them something to scroll
/// sideways for; above the split their position is fixed too, so widening the list does not walk
/// the delete across the screen.
pub(crate) fn mail_surface(bar: &SelectionBar, split: &gtk::Paned) -> gtk::Box {
    split.set_vexpand(true);
    let surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
    surface.append(bar.widget());
    surface.append(split);
    surface
}

/// A button whose action never changes with the selection.
fn action_button(
    sender: &relm4::Sender<AppInput>,
    action: BulkAction,
    destructive: bool,
) -> gtk::Button {
    let button = labelled_button(action_icon(action), action_label(action));
    if destructive {
        button.add_css_class("destructive-action");
    }
    let input = sender.clone();
    button.connect_clicked(move |_| input.emit(AppInput::ActOnSelection(action)));
    button
}

/// Icon and word together, which is what the bar's full width across both panes buys: the glyph
/// is what the eye finds, the word is what settles which of the two bins it is.
fn labelled_button(icon: &str, label: &str) -> gtk::Button {
    let content = adw::ButtonContent::new();
    content.set_icon_name(icon);
    content.set_label(label);
    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
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

/// The glyph for each action. Themed rather than bundled (Archive excepted, which is ours): a
/// name the theme lacks draws the broken-image icon while the bar carries on as though nothing
/// happened, so the widget tests assert each of these resolves rather than looking once.
pub(super) fn action_icon(action: BulkAction) -> &'static str {
    match action {
        BulkAction::MarkRead => "mail-read-symbolic",
        BulkAction::MarkUnread => "mail-unread-symbolic",
        BulkAction::Flag => "starred-symbolic",
        BulkAction::Unflag => "non-starred-symbolic",
        BulkAction::Archive => "mailcal-archive-symbolic",
        BulkAction::Delete => "user-trash-symbolic",
        // Its own glyph rather than a second bin: the two sit side by side, and the label is not
        // the only thing that should tell them apart.
        BulkAction::PermanentlyDelete => "edit-delete-symbolic",
    }
}

pub(super) const SELECT_ALL_ICON: &str = "edit-select-all-symbolic";
pub(super) const CLEAR_ICON: &str = "window-close-symbolic";

#[cfg(test)]
#[path = "selection_bar_tests.rs"]
pub(super) mod tests;
