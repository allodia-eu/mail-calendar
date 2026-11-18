//! The composer's importance picker (`docs/importance.md`): a menu in the action bar, beside the
//! signature's.
//!
//! The importance is the core's, held with the composition: the menu reads it there, a pick writes
//! it there, and every save and the send take it from there. So nothing here is sent anywhere, and
//! a composer moved into a window of its own opens with the importance the one it left had.

use std::{cell::Cell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{ComposerImportance, MailcalApp, MessageImportance};

use crate::l10n;

/// What the picker reads and writes: the app, or a test's fake.
pub(crate) trait ImportanceStore {
    fn read(&self, composition: &str) -> Option<ComposerImportance>;
    fn set(&self, composition: &str, importance: MessageImportance) -> Option<ComposerImportance>;
}

impl ImportanceStore for MailcalApp {
    fn read(&self, composition: &str) -> Option<ComposerImportance> {
        self.composer_importance(composition.to_owned()).ok()
    }

    fn set(&self, composition: &str, importance: MessageImportance) -> Option<ComposerImportance> {
        self.set_composer_importance(composition.to_owned(), importance)
            .ok()
    }
}

pub(crate) struct ImportanceControl {
    store: Arc<dyn ImportanceStore>,
    composition: String,
    button: gtk::MenuButton,
    action: gio::SimpleAction,
    /// The core's last answer, `(selected, changed)`. The importance moves only through `set`,
    /// which answers with the picker as it then stands.
    answered: Cell<(MessageImportance, bool)>,
}

impl ImportanceControl {
    pub(crate) fn new(store: Arc<dyn ImportanceStore>, composition: &str) -> Rc<Self> {
        let picker = store.read(composition);
        let (selected, changed) = picker
            .as_ref()
            .map_or((MessageImportance::Normal, false), |p| {
                (p.selected, p.changed)
            });
        let action = gio::SimpleAction::new_stateful(
            "choose",
            Some(glib::VariantTy::STRING),
            &token(selected).to_variant(),
        );
        let control = Rc::new(Self {
            store,
            composition: composition.to_owned(),
            button: gtk::MenuButton::new(),
            action,
            answered: Cell::new((selected, changed)),
        });
        let options = picker.map_or_else(Vec::new, |picker| picker.options);
        control.build(&options);
        control.show(selected);
        control
    }

    pub(crate) fn widget(&self) -> &gtk::MenuButton {
        &self.button
    }

    /// Whether the importance differs from what the composer opened with: its half of "was
    /// anything written", beside the files'.
    pub(crate) fn changed(&self) -> bool {
        self.answered.get().1
    }

    /// What the message goes out with now, for the autosave timer to notice a pick.
    pub(crate) fn selected(&self) -> MessageImportance {
        self.answered.get().0
    }

    fn build(self: &Rc<Self>, options: &[MessageImportance]) {
        // One stateful action every entry targets, so GTK draws the choice with its own radio mark
        // and announces it as selected.
        let menu = gio::Menu::new();
        for &importance in options {
            let item = gio::MenuItem::new(Some(label(importance)), None);
            item.set_action_and_target_value(
                Some("importance.choose"),
                Some(&token(importance).to_variant()),
            );
            menu.append_item(&item);
        }
        let group = gio::SimpleActionGroup::new();
        group.add_action(&self.action);
        self.button.set_menu_model(Some(&menu));
        self.button.insert_action_group("importance", Some(&group));
        self.button.set_always_show_arrow(true);

        let control = Rc::downgrade(self);
        self.action.connect_activate(move |_, target| {
            let Some(control) = control.upgrade() else {
                return;
            };
            let Some(importance) = target.and_then(glib::Variant::str).and_then(from_token) else {
                return;
            };
            let Some(picker) = control.store.set(&control.composition, importance) else {
                return;
            };
            control.answered.set((picker.selected, picker.changed));
            control.show(picker.selected);
        });
    }

    /// Marks the menu and labels the button with what is in force: the glyph a list row draws
    /// beside the word, so a raised or lowered message reads as one before it is sent.
    fn show(&self, selected: MessageImportance) {
        self.action.set_state(&token(selected).to_variant());
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let icon = match selected {
            MessageImportance::High => Some(super::icons::IMPORTANCE_HIGH),
            MessageImportance::Low => Some(super::icons::IMPORTANCE_LOW),
            MessageImportance::Normal => None,
        };
        if let Some(icon) = icon {
            content.append(&gtk::Image::from_icon_name(icon));
        }
        content.append(&gtk::Label::new(Some(l10n::compose_importance_label())));
        self.button.set_child(Some(&content));
        self.button.set_tooltip_text(Some(label(selected)));
    }
}

pub(crate) fn label(importance: MessageImportance) -> &'static str {
    match importance {
        MessageImportance::High => l10n::compose_importance_high(),
        MessageImportance::Normal => l10n::compose_importance_normal(),
        MessageImportance::Low => l10n::compose_importance_low(),
    }
}

/// The action's target. A word rather than an index, so the menu cannot pick the wrong entry if
/// the core reorders its options.
fn token(importance: MessageImportance) -> &'static str {
    match importance {
        MessageImportance::High => "high",
        MessageImportance::Normal => "normal",
        MessageImportance::Low => "low",
    }
}

fn from_token(token: &str) -> Option<MessageImportance> {
    match token {
        "high" => Some(MessageImportance::High),
        "normal" => Some(MessageImportance::Normal),
        "low" => Some(MessageImportance::Low),
        _ => None,
    }
}

#[cfg(test)]
#[path = "composer_importance_tests.rs"]
pub(crate) mod tests;
