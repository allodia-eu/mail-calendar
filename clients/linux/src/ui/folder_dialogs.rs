//! The three dialogs a folder's row menu opens: a name (new or renamed folder), a destination
//! (Move to…), and the delete confirmation (`docs/folder-pane.md`, rules 24 to 26); and the
//! destination a message's Move to folder… asks for.
//!
//! Each is a transient modal over the window the row sits in, and each answers with one
//! `FolderIntent`; none keeps state after it closes.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{FolderIntent, FolderNameCheck, SelectedRow};

use super::{
    AppInput,
    folder_actions::{FolderInput, NameCheck, NameQuery, delete_copy, name_problem},
    folder_pane_rows::INDENT,
    folder_picker::{PickerRow, keyboard_target},
    message_move::MessageMove,
    modal,
};
use crate::l10n;

/// What a name dialog is for.
pub(crate) enum NamePurpose {
    /// A new folder inside `parent`, or at the top of the account's tree.
    Create { parent: Option<String> },
    /// A new name for `key`, which sits inside `parent`.
    Rename {
        key: String,
        parent: Option<String>,
        current: String,
    },
}

fn window_of(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_downcast::<gtk::Window>()
}

fn content_box() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content
}

/// Cancel and a confirm button, trailing. The confirm is returned so the caller can wire it.
fn action_bar(window: &gtk::Window, confirm: &str, destructive: bool) -> (gtk::Box, gtk::Button) {
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    actions.append(&cancel);
    let confirm = gtk::Button::with_label(confirm);
    confirm.add_css_class(if destructive {
        "destructive-action"
    } else {
        "suggested-action"
    });
    actions.append(&confirm);
    (actions, confirm)
}

/// Asks for a folder name, checking it with the core as it is typed (rule 25).
pub(crate) fn name_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    purpose: NamePurpose,
    check: &NameCheck,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let (title, confirm_label, initial) = match &purpose {
        NamePurpose::Create { .. } => (l10n::folder_new_title(), l10n::action_create(), ""),
        NamePurpose::Rename { current, .. } => (
            l10n::folder_rename_title(),
            l10n::action_save(),
            current.as_str(),
        ),
    };
    let (window, _) = modal::new(&parent_window, title, 400, None);
    window.set_resizable(false);
    let content = content_box();
    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_placeholder_text(Some(l10n::folder_name_label()));
    entry.update_property(&[AccessibleProperty::Label(l10n::folder_name_label())]);
    content.append(&entry);
    let problem = gtk::Label::new(None);
    problem.set_xalign(0.0);
    problem.set_wrap(true);
    problem.add_css_class("error");
    problem.set_visible(false);
    content.append(&problem);
    let (actions, confirm) = action_bar(&window, confirm_label, false);
    content.append(&actions);
    window.set_child(Some(&content));

    let account = account.to_owned();
    let (parent, renaming) = match &purpose {
        NamePurpose::Create { parent } => (parent.clone(), None),
        NamePurpose::Rename { key, parent, .. } => (parent.clone(), Some(key.clone())),
    };
    let checked = {
        let check = check.clone();
        let (account, parent, renaming) = (account.clone(), parent.clone(), renaming.clone());
        move |name: &str| {
            check(&NameQuery {
                account: &account,
                parent: parent.as_deref(),
                name,
                renaming: renaming.as_deref(),
            })
        }
    };
    let show = {
        let (confirm, problem) = (confirm.clone(), problem.clone());
        move |answer: FolderNameCheck| {
            confirm.set_sensitive(answer == FolderNameCheck::Valid);
            problem.set_text(name_problem(answer).unwrap_or_default());
            problem.set_visible(name_problem(answer).is_some());
        }
    };
    show(checked(initial));
    {
        let (checked, show) = (checked.clone(), show.clone());
        entry.connect_changed(move |entry| show(checked(&entry.text())));
    }
    let submit = {
        let (entry, window, sender) = (entry.clone(), window.clone(), sender.clone());
        move || {
            let name = entry.text().to_string();
            if checked(&name) != FolderNameCheck::Valid {
                return;
            }
            let intent = match &purpose {
                NamePurpose::Create { parent } => FolderIntent::Create {
                    account: account.clone(),
                    parent: parent.clone(),
                    name,
                },
                NamePurpose::Rename { key, .. } => FolderIntent::Rename {
                    account: account.clone(),
                    key: key.clone(),
                    name,
                },
            };
            sender.emit(AppInput::Folder(FolderInput::Change(intent)));
            window.close();
        }
    };
    let submit = std::rc::Rc::new(submit);
    {
        let submit = submit.clone();
        confirm.connect_clicked(move |_| submit());
    }
    entry.connect_activate(move |_| submit());
    window.present();
    entry.grab_focus();
}

/// Lists where a folder may go (rule 24) and moves it to the one picked.
pub(crate) fn move_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    key: &str,
    name: &str,
    candidates: &[PickerRow],
    sender: &relm4::Sender<AppInput>,
) {
    let (sender, account, key) = (sender.clone(), account.to_owned(), key.to_owned());
    let title = l10n::folder_move_title(name);
    destination_dialog(anchor, &title, candidates, move |parent| {
        sender.emit(AppInput::Folder(FolderInput::Change(FolderIntent::Move {
            account: account.clone(),
            key: key.clone(),
            parent,
        })));
    });
}

/// Lists the folders a message, or the selection it belongs to, may be filed in (rule 24) and
/// moves it to the one picked. The model decides at that moment whether `row` stands for the
/// selection, as it does for a drop.
pub(crate) fn message_move_dialog(
    anchor: &impl IsA<gtk::Widget>,
    row: SelectedRow,
    plan: MessageMove,
    sender: &relm4::Sender<AppInput>,
) {
    let (sender, account) = (sender.clone(), plan.account);
    destination_dialog(
        anchor,
        l10n::message_move_title(),
        &plan.choices,
        move |key| {
            // Every row this picker draws stands for a folder: only Move to… has a Top level.
            let Some(key) = key else {
                return;
            };
            sender.emit(AppInput::Folder(FolderInput::MoveMail {
                row: row.clone(),
                account: account.clone(),
                key,
            }));
        },
    );
}

/// The tallest the list grows before it scrolls inside the dialog.
const PICKER_MAX_HEIGHT: i32 = 420;

/// A modal list of `rows` under `title`, drawn as the pane draws its tree, calling `chosen` with
/// the key of the row picked (`None` for Top level). A disabled row cannot be picked.
fn destination_dialog(
    anchor: &impl IsA<gtk::Widget>,
    title: &str,
    rows: &[PickerRow],
    chosen: impl Fn(Option<String>) + 'static,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let (window, _) = modal::new(&parent_window, title, 400, None);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.update_property(&[AccessibleProperty::Label(title)]);
    list.set_valign(gtk::Align::Start);
    // Inside the scrolled area, so the card's edge is not clipped and the list scrolls the way a
    // preferences page does.
    list.set_margin_top(24);
    list.set_margin_bottom(6);
    list.set_margin_start(24);
    list.set_margin_end(24);
    for row in rows {
        list.append(&picker_row(row));
    }
    walk_destinations_only(&list, rows);
    {
        let keys: Vec<Option<String>> = rows.iter().map(|row| row.key.clone()).collect();
        let window = window.clone();
        list.connect_row_activated(move |_, row| {
            let picked = usize::try_from(row.index())
                .ok()
                .and_then(|index| keys.get(index));
            if let Some(key) = picked {
                chosen(key.clone());
                window.close();
            }
        });
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_propagate_natural_height(true);
    scroll.set_max_content_height(PICKER_MAX_HEIGHT);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.set_halign(gtk::Align::End);
    cancel.set_margin_top(12);
    cancel.set_margin_bottom(24);
    cancel.set_margin_end(24);
    let dialog = window.clone();
    cancel.connect_clicked(move |_| dialog.close());
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&scroll);
    content.append(&cancel);
    window.set_child(Some(&content));
    window.present();
    if let Some(first) = rows
        .iter()
        .position(|row| row.enabled)
        .and_then(|index| i32::try_from(index).ok())
        .and_then(|index| list.row_at_index(index))
    {
        first.grab_focus();
    }
}

/// One picker row, laid out as a pane row: the role's icon and the folder's own name, moved in by
/// the pane's indent step. A row that is not a destination is insensitive, which dims it, takes it
/// out of the focus chain and reports it disabled to assistive technology.
fn picker_row(target: &PickerRow) -> gtk::ListBoxRow {
    let icon = gtk::Image::from_icon_name(target.icon);
    icon.set_valign(gtk::Align::Center);
    // A folder's name is the server's text; a plain label never reads it as markup.
    let name = gtk::Label::new(Some(&target.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_valign(gtk::Align::Center);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    line.set_size_request(-1, ROW_HEIGHT);
    line.set_margin_start(12 + INDENT * i32::try_from(target.indent).unwrap_or(0));
    line.set_margin_end(12);
    line.append(&icon);
    line.append(&name);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&line));
    row.set_selectable(false);
    row.set_activatable(target.enabled);
    // The name alone would lose where the folder sits, which the indent shows and nothing reads.
    row.update_property(&[AccessibleProperty::Label(&target.label)]);
    if !target.enabled {
        row.set_sensitive(false);
        row.update_state(&[gtk::accessible::State::Disabled(true)]);
    }
    row
}

/// A one-line row's height in a libadwaita boxed list, so a picker row stands as tall as the
/// action rows the pane is built from.
const ROW_HEIGHT: i32 = 50;

/// Moves the arrow keys, Page Up/Down, Home and End over the rows that can be chosen only.
///
/// `GtkListBox` steps its cursor onto every row, and onto an insensitive one it cannot move focus,
/// so the key does nothing visible and Enter then picks the row focus stayed on.
fn walk_destinations_only(list: &gtk::ListBox, rows: &[PickerRow]) {
    let enabled: Vec<bool> = rows.iter().map(|row| row.enabled).collect();
    let every_row = i32::try_from(enabled.len()).unwrap_or(i32::MAX);
    list.connect_move_cursor(move |list, step, count, _, _| {
        let steps = match step {
            gtk::MovementStep::DisplayLines => count,
            gtk::MovementStep::Pages => count.saturating_mul(PICKER_MAX_HEIGHT / ROW_HEIGHT),
            gtk::MovementStep::BufferEnds => count.saturating_mul(every_row),
            _ => return,
        };
        let Some(from) = list
            .focus_child()
            .and_then(|row| usize::try_from(row.downcast::<gtk::ListBoxRow>().ok()?.index()).ok())
        else {
            return;
        };
        list.stop_signal_emission_by_name("move-cursor");
        if let Some(row) = keyboard_target(&enabled, from, steps)
            .and_then(|index| i32::try_from(index).ok())
            .and_then(|index| list.row_at_index(index))
        {
            row.grab_focus();
            return;
        }
        // Past the last row, focus leaves the list as GTK's own handler would send it.
        let (direction, onward) = if steps < 0 {
            (gtk::DirectionType::Up, gtk::DirectionType::TabBackward)
        } else {
            (gtk::DirectionType::Down, gtk::DirectionType::TabForward)
        };
        if !list.keynav_failed(direction)
            && let Some(root) = list.root()
        {
            root.child_focus(onward);
        }
    });
}

/// Confirms a delete, worded by whether the folder is already in Trash (rule 26).
pub(crate) fn delete_dialog(
    anchor: &impl IsA<gtk::Widget>,
    account: &str,
    key: &str,
    name: &str,
    in_trash: bool,
    sender: &relm4::Sender<AppInput>,
) {
    let Some(parent_window) = window_of(anchor) else {
        return;
    };
    let copy = delete_copy(name, in_trash);
    let (window, _) = modal::new(&parent_window, &copy.title, 420, None);
    window.set_resizable(false);
    let content = content_box();
    let message = gtk::Label::new(Some(copy.message));
    message.set_wrap(true);
    message.set_xalign(0.0);
    content.append(&message);
    let (actions, confirm) = action_bar(&window, copy.confirm, copy.destructive);
    content.append(&actions);
    window.set_child(Some(&content));
    let (sender, dialog) = (sender.clone(), window.clone());
    let (account, key) = (account.to_owned(), key.to_owned());
    confirm.connect_clicked(move |_| {
        sender.emit(AppInput::Folder(FolderInput::Change(
            FolderIntent::Delete {
                account: account.clone(),
                key: key.clone(),
            },
        )));
        dialog.close();
    });
    window.present();
}

#[cfg(test)]
pub(crate) mod widget_tests {
    use adw::prelude::*;

    use super::{INDENT, picker_row, walk_destinations_only};
    use crate::ui::{folder_picker::PickerRow, icons};

    fn target(enabled: bool) -> PickerRow {
        PickerRow {
            key: Some("q1".to_owned()),
            name: "Q1 & <b>co</b>".to_owned(),
            icon: icons::FOLDER,
            indent: 2,
            enabled,
            label: "Work / Q1 & <b>co</b>".to_owned(),
        }
    }

    pub(crate) fn a_picker_row_reads_as_a_pane_row_and_only_a_destination_responds() {
        let row = picker_row(&target(true));
        assert!(row.is_activatable() && row.is_sensitive());
        let line = row
            .child()
            .and_downcast::<gtk::Box>()
            .expect("the row's line");
        assert_eq!(
            line.margin_start(),
            12 + 2 * INDENT,
            "two of the pane's steps"
        );
        let name = line
            .last_child()
            .and_downcast::<gtk::Label>()
            .expect("the folder's name");
        assert_eq!(name.text(), "Q1 & <b>co</b>", "the server's text, as text");
        assert!(!name.uses_markup());

        let ancestor = picker_row(&target(false));
        assert!(!ancestor.is_activatable());
        assert!(
            !ancestor.is_sensitive(),
            "dimmed, and out of the focus chain"
        );
    }

    pub(crate) fn the_arrow_keys_pass_over_a_row_that_cannot_be_chosen() {
        // Inbox, Archive kept as a parent, and 2026 inside it.
        let rows = [target(true), target(false), target(true)];
        let list = gtk::ListBox::new();
        for row in &rows {
            list.append(&picker_row(row));
        }
        walk_destinations_only(&list, &rows);
        let window = gtk::Window::new();
        window.set_child(Some(&list));
        let row = |index| list.row_at_index(index).expect("a picker row");
        assert!(row(0).grab_focus());

        list.emit_move_cursor(gtk::MovementStep::DisplayLines, 1, false, false);
        let focused = || RootExt::focus(&window).and_downcast::<gtk::ListBoxRow>();
        assert_eq!(focused(), Some(row(2)), "one press of Down reaches 2026");
        list.emit_move_cursor(gtk::MovementStep::DisplayLines, -1, false, false);
        assert_eq!(
            focused(),
            Some(row(0)),
            "and one press of Up returns to Inbox"
        );
    }
}
