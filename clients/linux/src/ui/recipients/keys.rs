//! The keys a recipient field answers ahead of its entry: the history's chords, the suggestion
//! list's Escape, Up, Down and Enter, and Enter over a typed address.

use std::rc::Rc;

use gtk::{gdk, glib, prelude::*};

use super::{super::history, Inner};

impl Inner {
    pub(super) fn key_pressed(
        self: &Rc<Self>,
        key: gdk::Key,
        keycode: u32,
        state: gdk::ModifierType,
    ) -> glib::Propagation {
        let named = history::chord_key(&self.entry.display(), key, keycode);
        if let Some(step) = history::chord(named, state) {
            self.step(step);
            return glib::Propagation::Stop;
        }
        let enter = matches!(key, gdk::Key::Return | gdk::Key::KP_Enter);
        if self.popover.is_visible() {
            match key {
                // An overlay with no keyboard way out is a trap, and the composer binds Escape to
                // nothing else; it is a pane, not a dialog.
                gdk::Key::Escape => {
                    self.popover.popdown();
                    return glib::Propagation::Stop;
                }
                gdk::Key::Down => return self.step_selection(1),
                gdk::Key::Up => return self.step_selection(-1),
                _ if enter => {
                    if let Some(row) = self.list.selected_row() {
                        self.accept(row.index());
                        return glib::Propagation::Stop;
                    }
                }
                _ => {}
            }
        }
        // Nothing picked, so Enter finishes what is typed, or is the owner's.
        if enter && !self.owner_activates.get() && self.finish_typed() {
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    }

    fn step_selection(&self, step: i32) -> glib::Propagation {
        let count = i32::try_from(self.offered.borrow().len()).unwrap_or(0);
        if count == 0 {
            return glib::Propagation::Proceed;
        }
        let next = match self.list.selected_row() {
            Some(row) => (row.index() + step).clamp(0, count - 1),
            // Arriving from the entry: Down takes the first, Up the last.
            None if step > 0 => 0,
            None => count - 1,
        };
        self.list.select_row(self.list.row_at_index(next).as_ref());
        glib::Propagation::Stop
    }
}
