//! What the Jitsi server field shows, what it stores and what it says about an address the core
//! refuses.
//!
//! The row commits on apply (Return or its apply button) and on losing focus; these assertions
//! drive the apply directly. Called from the crate's single `gtk::init` test.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::jitsi_server_group;
use crate::l10n;

/// A stand-in for the core's store: it keeps an `https` address with its trailing `/` dropped,
/// and refuses anything else.
fn store() -> Rc<RefCell<Option<String>>> {
    Rc::new(RefCell::new(None))
}

fn group_over(held: &Rc<RefCell<Option<String>>>) -> adw::PreferencesGroup {
    let read = Rc::clone(held);
    let write = Rc::clone(held);
    jitsi_server_group(
        move || read.borrow().clone(),
        move |typed| {
            let typed = typed.trim().to_owned();
            if typed.is_empty() {
                *write.borrow_mut() = None;
                return true;
            }
            if !typed.starts_with("https://") || typed.contains(['?', '#']) {
                return false;
            }
            *write.borrow_mut() = Some(typed.trim_end_matches('/').to_owned());
            true
        },
    )
}

fn parts(group: &adw::PreferencesGroup) -> (adw::EntryRow, gtk::Label) {
    let widgets = crate::ui::setup_widget_tests::descendants::<gtk::Widget>(
        group.upcast_ref::<gtk::Widget>(),
    );
    let entry = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<adw::EntryRow>().ok())
        .expect("the group holds the field");
    let refused = widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk::Label>().ok())
        .find(|label| label.text() == l10n::settings_jitsi_server_invalid())
        .expect("the group holds the refusal");
    (entry, refused)
}

pub(crate) fn the_jitsi_server_field_stores_what_the_core_accepts_and_says_when_it_refuses() {
    let held = store();
    let group = group_over(&held);
    let (entry, refused) = parts(&group);
    assert_eq!(
        entry.text(),
        "",
        "the field is empty until the person fills it"
    );
    assert!(!refused.is_visible());

    entry.set_text("meet.example.org");
    entry.emit_by_name::<()>("apply", &[]);
    assert!(refused.is_visible(), "an address the core refuses is said");
    assert_eq!(*held.borrow(), None, "and changes nothing");

    entry.set_text("https://meet.example.org/");
    assert!(!refused.is_visible(), "typing again takes the refusal away");
    entry.emit_by_name::<()>("apply", &[]);
    assert_eq!(held.borrow().as_deref(), Some("https://meet.example.org"));
    assert_eq!(
        entry.text(),
        "https://meet.example.org",
        "the stored form is shown"
    );
    assert!(!refused.is_visible());

    let reopened = group_over(&held);
    assert_eq!(parts(&reopened).0.text(), "https://meet.example.org");

    entry.set_text("");
    entry.emit_by_name::<()>("apply", &[]);
    assert_eq!(*held.borrow(), None, "an empty field forgets the server");
}
