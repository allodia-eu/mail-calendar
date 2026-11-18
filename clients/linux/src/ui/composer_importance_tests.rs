//! The importance menu drawn from the core's answer. The GTK half is called from the crate's single
//! `gtk::init` test (see [`super::super::mailbox::tests`]).

use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{ComposerImportance, MessageImportance};

use super::{ImportanceControl, ImportanceStore, from_token, token};

/// A stand-in for the core's picker: normal to start with, highest first.
struct FakeStore(Mutex<MessageImportance>);

impl FakeStore {
    fn picker(selected: MessageImportance) -> ComposerImportance {
        ComposerImportance {
            selected,
            options: vec![
                MessageImportance::High,
                MessageImportance::Normal,
                MessageImportance::Low,
            ],
            changed: selected != MessageImportance::Normal,
        }
    }
}

impl ImportanceStore for FakeStore {
    fn read(&self, _composition: &str) -> Option<ComposerImportance> {
        Some(Self::picker(*self.0.lock().unwrap()))
    }

    fn set(&self, _composition: &str, importance: MessageImportance) -> Option<ComposerImportance> {
        *self.0.lock().unwrap() = importance;
        self.read("")
    }
}

#[test]
fn every_importance_survives_the_menus_target() {
    for importance in [
        MessageImportance::High,
        MessageImportance::Normal,
        MessageImportance::Low,
    ] {
        assert_eq!(from_token(token(importance)), Some(importance));
    }
    assert_eq!(from_token("urgent"), None);
}

/// The menu lists what the core offers, in its order, and a pick goes to the core: what the
/// control then reports is the core's answer, not a value of its own.
pub(crate) fn a_pick_goes_through_the_core() {
    let store = Arc::new(FakeStore(Mutex::new(MessageImportance::Normal)));
    let control = ImportanceControl::new(Arc::clone(&store) as Arc<dyn ImportanceStore>, "c");
    let menu = control
        .widget()
        .menu_model()
        .expect("the picker has a menu");
    assert_eq!(
        menu.n_items(),
        3,
        "one entry per importance the core offers"
    );
    assert!(!control.changed());

    control
        .action
        .activate(Some(&token(MessageImportance::High).to_variant()));
    assert_eq!(*store.0.lock().unwrap(), MessageImportance::High);
    assert_eq!(control.selected(), MessageImportance::High);
    assert!(control.changed(), "a raised message is something written");
    assert_eq!(
        control
            .action
            .state()
            .and_then(|state| state.get::<String>()),
        Some("high".to_owned()),
        "the radio mark moves to the pick"
    );
    let _ = glib::MainContext::default().iteration(false);
}
