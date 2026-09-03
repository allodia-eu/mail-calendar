//! What the message list does with a click and a keystroke while rows are being selected, and how
//! the model's selection is drawn back onto the `GtkListBox`.
//!
//! Split from [`super::shell`] to keep each file under the 500-line limit. It is here rather than
//! beside the bar because none of it is the bar: it is the list's own input, and the one place
//! that decides what a modified click means.

use adw::prelude::*;
use mailcal_bindings::BulkAction;

use super::{AppInput, AppModel, selection::SelectMode};

/// Reads Ctrl- and Shift-clicks on the message list, which `GtkListBox` cannot give us.
///
/// Its own click handler takes the modifier path only when `activate-on-single-click` is off, and
/// that flag is what opens a message with one click; with it on, a Ctrl-click selects and
/// *activates*, so it would open the row it was meant to add. Claiming the sequence in the capture
/// phase stops that handler running at all. A plain click is left to it, having first told the
/// model to start a fresh selection, so clicking a row still opens it.
pub(super) fn selection_gesture(messages: &gtk::ListBox, sender: &relm4::Sender<AppInput>) {
    let click = gtk::GestureClick::new();
    // The primary button only. A `GestureClick` listens to every button by default, so a
    // right-click aimed at a row's menu, or a stray middle-click, would otherwise arrive here as a
    // plain click and collapse a selection the user had just built.
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let input = sender.clone();
    let list = messages.clone();
    click.connect_pressed(move |gesture, _, _, y| {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a widget coordinate fits an i32"
        )]
        let Some(row) = list.row_at_y(y as i32) else {
            return;
        };
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

/// Draws the model's selection onto the list.
///
/// The widget owns no selection: GTK moves its own on every click, and this is the one place that
/// decides what is selected, so the two cannot drift into disagreeing about which rows an action
/// would reach.
pub(super) fn sync_selection(messages: &gtk::ListBox, model: &AppModel) {
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
