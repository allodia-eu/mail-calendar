//! A recipient field's undo and redo (`docs/undo.md`).
//!
//! The field's own, not the entry's. The entry holds only the address being typed, and the field
//! rewrites it whenever an address is finished, so the entry's history would type a finished
//! address back in and finish it a second time. A step here is the whole field before a change:
//! typing one address, finishing it, accepting a suggestion, removing a pill.

use gtk::gdk;

use super::tokens;

/// The most steps kept, oldest dropped first.
const MAX_STEPS: usize = 100;

/// What an undo chord asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Undo,
    Redo,
}

/// Ctrl+Z is undo, Shift+Ctrl+Z and Ctrl+Y redo: the chords GTK binds in every text field.
pub(super) fn chord(key: gdk::Key, state: gdk::ModifierType) -> Option<Step> {
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
    if !ctrl || state.contains(gdk::ModifierType::ALT_MASK) {
        return None;
    }
    match key.to_lower() {
        gdk::Key::z if shift => Some(Step::Redo),
        gdk::Key::z => Some(Step::Undo),
        gdk::Key::y if !shift => Some(Step::Redo),
        _ => None,
    }
}

/// The values the field held before each change, and after each undo.
#[derive(Debug, Default)]
pub(super) struct FieldHistory {
    undo: Vec<String>,
    redo: Vec<String>,
    /// Whether the newest step is an address still being typed, which the next keystroke joins.
    typing: bool,
}

impl FieldHistory {
    /// Files a change from `before` to `after`. `typed` is a keystroke in the entry, which joins
    /// the step before it while the same address is being typed; finishing it is a step of its
    /// own.
    pub(super) fn record(&mut self, before: &str, after: &str, typed: bool) {
        let same_recipients = tokens::committed(before) == tokens::committed(after);
        if !(typed && self.typing && same_recipients) {
            self.undo.push(before.to_owned());
            if self.undo.len() > MAX_STEPS {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = typed && same_recipients;
    }

    /// The value to go back to from `current`, if there is one.
    pub(super) fn undo(&mut self, current: &str) -> Option<String> {
        let value = self.undo.pop()?;
        self.redo.push(current.to_owned());
        self.typing = false;
        Some(value)
    }

    /// The value an undo left behind, if nothing has been changed since.
    pub(super) fn redo(&mut self, current: &str) -> Option<String> {
        let value = self.redo.pop()?;
        self.undo.push(current.to_owned());
        self.typing = false;
        Some(value)
    }

    /// Forgets everything: the field was given a value from outside it.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use gtk::gdk::{Key, ModifierType};

    use super::{FieldHistory, Step, chord};

    const CTRL: ModifierType = ModifierType::CONTROL_MASK;

    /// Types `text` one character at a time onto `field`, as the entry reports it.
    fn type_into(history: &mut FieldHistory, field: &mut String, text: &str) {
        for letter in text.chars() {
            let before = field.clone();
            field.push(letter);
            history.record(&before, field, true);
        }
    }

    #[test]
    fn one_address_typed_is_one_step() {
        let mut history = FieldHistory::default();
        let mut field = String::new();
        type_into(&mut history, &mut field, "bob@example.test");
        assert_eq!(history.undo(&field).as_deref(), Some(""));
    }

    #[test]
    fn finishing_an_address_is_a_step_of_its_own() {
        let mut history = FieldHistory::default();
        let mut field = String::new();
        type_into(&mut history, &mut field, "bob@example.test,");
        type_into(&mut history, &mut field, " carol");
        let field = field.as_str();
        let back = history.undo(field).unwrap();
        assert_eq!(back, "bob@example.test,");
        assert_eq!(history.undo(&back).as_deref(), Some("bob@example.test"));
    }

    #[test]
    fn a_removed_recipient_comes_back_and_goes_again() {
        let mut history = FieldHistory::default();
        history.record(
            "bob@example.test, carol@example.test, ",
            "carol@example.test, ",
            false,
        );
        let back = history.undo("carol@example.test, ").unwrap();
        assert_eq!(back, "bob@example.test, carol@example.test, ");
        assert_eq!(history.redo(&back).as_deref(), Some("carol@example.test, "));
    }

    #[test]
    fn a_change_after_an_undo_leaves_nothing_to_redo() {
        let mut history = FieldHistory::default();
        history.record("", "bob@example.test, ", false);
        history.undo("bob@example.test, ");
        history.record("", "carol@example.test, ", false);
        assert_eq!(history.redo("carol@example.test, "), None);
    }

    #[test]
    fn typing_after_an_undo_starts_a_new_step() {
        let mut history = FieldHistory::default();
        let mut field = String::new();
        type_into(&mut history, &mut field, "bo");
        field = history.undo(&field).unwrap();
        type_into(&mut history, &mut field, "al");
        assert_eq!(history.undo(&field).as_deref(), Some(""));
        assert_eq!(history.undo(""), None);
    }

    #[test]
    fn a_field_given_a_value_forgets_its_history() {
        let mut history = FieldHistory::default();
        history.record("", "bob@example.test, ", false);
        history.clear();
        assert_eq!(history.undo("bob@example.test, "), None);
    }

    #[test]
    fn the_chords_are_gtks_own() {
        assert_eq!(chord(Key::z, CTRL), Some(Step::Undo));
        assert_eq!(
            chord(Key::Z, CTRL | ModifierType::SHIFT_MASK),
            Some(Step::Redo)
        );
        assert_eq!(
            chord(Key::z, CTRL | ModifierType::SHIFT_MASK),
            Some(Step::Redo)
        );
        assert_eq!(chord(Key::y, CTRL), Some(Step::Redo));
        assert_eq!(chord(Key::z, ModifierType::empty()), None);
        assert_eq!(chord(Key::z, CTRL | ModifierType::ALT_MASK), None);
        assert_eq!(chord(Key::v, CTRL), None);
    }
}
