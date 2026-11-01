//! What the message list does with a click and a keystroke while rows are being selected, and how
//! the model's selection is drawn back onto the `GtkListBox`.
//!
//! Split from [`super::shell`] to keep each file under the 500-line limit. It is here rather than
//! beside the bar because none of it is the bar: it is the list's own input, and the one place
//! that decides what a modified click means.

use std::collections::HashSet;

use adw::prelude::*;
use mailcal_bindings::BulkAction;

use super::{
    AppInput, AppModel, mailbox::ThreadKey, mailbox_display::DisplayRow,
    mailbox_progressive::ProgressiveRenderer, selection::SelectMode,
};

/// Reads Ctrl- and Shift-clicks on the message list, which `GtkListBox` cannot give us.
///
/// Its own click handler takes the modifier path only when `activate-on-single-click` is off, and
/// that flag is what opens a message with one click; with it on, a Ctrl-click selects and
/// *activates*, so it would open the row it was meant to add. Claiming the sequence in the capture
/// phase stops that handler running at all. A plain click is left to it, having first told the
/// model to start a fresh selection, so clicking a row still opens it.
///
/// A press on a button inside a row is the button's, not a click on the row: opening a row's menu
/// leaves the selection as it was, so the menu over a selected row reaches the whole selection
/// (`docs/list-selection.md`, rule 12).
pub(super) fn selection_gesture(messages: &gtk::ListBox, sender: &relm4::Sender<AppInput>) {
    let click = gtk::GestureClick::new();
    // The primary button only. A `GestureClick` listens to every button by default, so a
    // right-click aimed at a row's menu, or a stray middle-click, would otherwise arrive here as a
    // plain click and collapse a selection the user had just built.
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let input = sender.clone();
    let list = messages.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a widget coordinate fits an i32"
        )]
        let Some(row) = list.row_at_y(y as i32) else {
            return;
        };
        if pressed_on_control(&list, x, y) {
            return;
        }
        let Ok(index) = usize::try_from(row.index()) else {
            return;
        };
        let state = gesture.current_event_state();
        let mode = if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
            SelectMode::Range
        } else if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            SelectMode::Toggle
        } else {
            SelectMode::Replace
        };
        input.emit(AppInput::SelectRow { index, mode });
        if mode != SelectMode::Replace {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    messages.add_controller(click);
}

/// Whether the press at `x`, `y` in `list` landed on a button inside one of its rows.
pub(super) fn pressed_on_control(list: &gtk::ListBox, x: f64, y: f64) -> bool {
    let mut widget = list.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(current) = widget {
        if current == *list.upcast_ref::<gtk::Widget>() {
            return false;
        }
        if current.is::<gtk::Button>() {
            return true;
        }
        widget = current.parent();
    }
    false
}

/// The message list's keys: Delete (and Backspace) to Trash, Escape to clear, Ctrl+A to select
/// what is loaded.
///
/// Bound on the list rather than app-wide, and in the capture phase so the list's own Ctrl+A
/// binding does not select rows the model then has to correct. Nothing here reaches a focused
/// search field or composer, which is the whole reason for the scope
/// (`docs/list-selection.md`, rule 9).
pub(super) fn selection_keys(messages: &gtk::ListBox, sender: &relm4::Sender<AppInput>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let input = sender.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let message = match key {
            gtk::gdk::Key::Delete | gtk::gdk::Key::BackSpace => {
                Some(AppInput::ActOnSelection(BulkAction::Delete))
            }
            gtk::gdk::Key::Escape => Some(AppInput::ClearSelection),
            gtk::gdk::Key::a | gtk::gdk::Key::A
                if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) =>
            {
                Some(AppInput::SelectAllRows)
            }
            _ => None,
        };
        match message {
            Some(message) => {
                input.emit(message);
                gtk::glib::Propagation::Stop
            }
            None => gtk::glib::Propagation::Proceed,
        }
    });
    messages.add_controller(keys);
}

/// Draws the model's selection, and the conversations it keeps expanded, onto the list.
pub(super) fn sync_list(messages: &gtk::ListBox, model: &AppModel, drawn: &ProgressiveRenderer) {
    sync_selection(messages, model);
    drawn.sync_expansion(messages, &model.expanded_threads);
}

/// Draws the model's selection onto the list.
///
/// The widget owns no selection: GTK moves its own on every click, and this is the one place that
/// decides what is selected, so the two cannot drift into disagreeing about which rows an action
/// would reach.
fn sync_selection(messages: &gtk::ListBox, model: &AppModel) {
    let mut index = 0;
    while let Some(widget) = messages.row_at_index(index) {
        let selected = usize::try_from(index)
            .ok()
            .and_then(|index| model.snapshot.rows.get(index))
            .is_some_and(|row| model.selection.contains(row));
        if selected {
            messages.select_row(Some(&widget));
        } else {
            messages.unselect_row(&widget);
        }
        index += 1;
    }
}

/// Draws the model's record of which conversations are expanded onto their rows, so leaving a
/// conversation folds it away (`docs/row-actions.md`, rule 8).
///
/// `drawn` is what the list holds, row for row, which is the renderer's record rather than the
/// snapshot's: while a long list is still being built the two differ, and a row expanded under
/// another conversation's name would report itself and open that one. A row reports its own
/// disclosure back, and the model ignores a report of what it already holds, so drawing it here
/// starts nothing.
pub(super) fn sync_expansion(
    messages: &gtk::ListBox,
    drawn: &[DisplayRow],
    expanded: &HashSet<ThreadKey>,
) {
    for (index, row) in drawn.iter().enumerate() {
        let Some(thread) = &row.thread else {
            continue;
        };
        let Some(widget) = i32::try_from(index)
            .ok()
            .and_then(|index| messages.row_at_index(index))
            .and_downcast::<adw::ExpanderRow>()
        else {
            continue;
        };
        let open = expanded.contains(thread);
        if widget.is_expanded() != open {
            widget.set_expanded(open);
        }
    }
}
