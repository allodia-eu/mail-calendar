//! What the Signatures category draws from the core's page, and what it must never draw.
//!
//! The two pickers' rules are pure and run as ordinary tests. The rendering half is asserted on
//! the **rendered** widgets and called from the crate's single `gtk::init` test (see
//! [`crate::ui::mailbox::tests`]): `ActionRow::title()` reads back the string it was handed
//! whatever became of the label, so asserting on the property is a green light for a blank row.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    AccountSignatures, ControlState, SignatureEntry, SignaturePicker, SignaturesPage,
};

use super::{Actions, named_row, page_content, slot_choice, slot_selection};
use crate::{
    l10n,
    ui::{
        controls::tests::spins,
        mailbox::tests::{glib_records, rendered_labels},
    },
};

#[test]
fn a_slot_is_drawn_on_what_the_account_assigned_and_none_leads() {
    let ids = ["id-work".to_owned(), "id-home".to_owned()];

    assert_eq!(slot_selection(None, &ids), 0, "unassigned reads as None");
    assert_eq!(slot_selection(Some("id-work"), &ids), 1);
    assert_eq!(slot_selection(Some("id-home"), &ids), 2);
}

#[test]
fn picking_the_first_entry_clears_the_slot_rather_than_assigning_the_first_signature() {
    let ids = ["id-work".to_owned(), "id-home".to_owned()];

    // Off by one here is silent and wrong in the worst way: every account would quietly send the
    // signature above the one its picker shows.
    assert_eq!(slot_choice(0, &ids), None);
    assert_eq!(slot_choice(1, &ids), Some("id-work".to_owned()));
    assert_eq!(slot_choice(2, &ids), Some("id-home".to_owned()));
    assert_eq!(slot_choice(3, &ids), None, "a stale index assigns nothing");
}

/// A signature's name and an account's address are text somebody else wrote, and both land in a
/// row title. A bare ampersand renders the row **blank** when it is parsed as markup, and a
/// markup-shaped name is *applied* rather than shown.
pub(crate) fn a_signatures_own_text_is_never_parsed_as_markup() {
    let (rows, records) = glib_records(|| {
        [
            named_row("Sales & Marketing"),
            named_row("ada&grace@example.test"),
            named_row("<b>Work</b>"),
        ]
    });

    // Rendering alone cannot see this half: libadwaita re-applies the labels when the flag flips,
    // so a row built the wrong way still reads correctly and only warns.
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "a name or an address must not be parsed as markup: {records:?}"
    );
    for row in &rows {
        assert!(!row.uses_markup());
    }
    let shown = rows
        .iter()
        .flat_map(|row| rendered_labels(row.upcast_ref::<gtk::Widget>()))
        .collect::<Vec<_>>();
    for expected in ["Sales & Marketing", "ada&grace@example.test", "<b>Work</b>"] {
        assert!(
            shown.iter().any(|text| text == expected),
            "{expected:?} must reach the screen as itself: {shown:?}"
        );
    }
}

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};
const RUNNING: ControlState = ControlState {
    enabled: false,
    running: true,
};

/// What the page's controls asked for, in order.
type Asked = Rc<RefCell<Vec<String>>>;

fn actions(asked: &Asked) -> Actions {
    let (edit, delete, assign) = (asked.clone(), asked.clone(), asked.clone());
    Actions {
        edit: Rc::new(move |id| edit.borrow_mut().push(format!("edit {id:?}"))),
        delete: Rc::new(move |id| delete.borrow_mut().push(format!("delete {id}"))),
        assign: Rc::new(move |account, slot, signature| {
            assign
                .borrow_mut()
                .push(format!("assign {account} {slot:?} {signature:?}"));
        }),
    }
}

fn descendants<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(wanted) = widget.downcast_ref::<T>() {
            found.push(wanted.clone());
        }
        found.extend(descendants::<T>(&widget));
        child = widget.next_sibling();
    }
    found
}

fn button<'a>(buttons: &'a [gtk::Button], label: &str) -> Vec<&'a gtk::Button> {
    buttons
        .iter()
        .filter(|button| {
            rendered_labels(button.upcast_ref())
                .iter()
                .any(|text| text == label)
        })
        .collect()
}

/// Every control is drawn from the page's own state: a deletion running spins its Delete and holds
/// everything else, and each slot is drawn on what the core says it holds. Drawing asks for
/// nothing; a pick asks for the signature picked.
pub(crate) fn the_page_is_drawn_from_the_cores_record_and_asks_only_what_the_person_did() {
    let entry = |id: &str, name: &str, delete| SignatureEntry {
        id: id.to_owned(),
        name: name.to_owned(),
        edit: HELD,
        delete,
    };
    let page = SignaturesPage {
        signatures: vec![
            entry("id-work", "Work", RUNNING),
            entry("id-home", "Home", HELD),
        ],
        accounts: vec![AccountSignatures {
            account_id: "account-1".to_owned(),
            email: "ada@test.local".to_owned(),
            new_message: SignaturePicker {
                selected: Some("id-home".to_owned()),
                control: HELD,
            },
            reply_forward: SignaturePicker {
                selected: None,
                control: FREE,
            },
        }],
        add: HELD,
        new_id: "id-new".to_owned(),
        save: HELD,
        outcome: None,
    };
    let asked = Asked::default();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page_content(&body, &page, &actions(&asked));
    let root = body.upcast_ref::<gtk::Widget>();
    assert!(asked.borrow().is_empty(), "drawing asked for nothing");

    let buttons = descendants::<gtk::Button>(root);
    let deletes = button(&buttons, l10n::settings_signatures_delete());
    assert_eq!(deletes.len(), 2);
    assert!(
        spins(deletes[0]) && !deletes[0].is_sensitive(),
        "its deletion runs"
    );
    assert!(
        !spins(deletes[1]) && !deletes[1].is_sensitive(),
        "held while it runs"
    );
    for held in button(&buttons, l10n::settings_signatures_edit())
        .into_iter()
        .chain(button(&buttons, l10n::settings_signatures_add()))
    {
        assert!(!held.is_sensitive() && !spins(held));
    }

    let slots = descendants::<adw::ComboRow>(root);
    assert_eq!(slots.len(), 2, "two slots for the one account");
    assert_eq!(slots[0].selected(), 2, "None, Work, Home: on Home");
    assert!(!slots[0].is_sensitive());
    assert_eq!(slots[1].selected(), 0, "on None");
    assert!(slots[1].is_sensitive());
    slots[1].set_selected(1);
    slots[1].set_selected(0);
    assert_eq!(
        *asked.borrow(),
        [
            "assign account-1 ReplyForward Some(\"id-work\")",
            "assign account-1 ReplyForward None",
        ]
    );
}

/// With nothing written and a free page, New signature opens the editor on a new signature.
pub(crate) fn an_empty_library_says_so_and_new_signature_opens_the_editor() {
    let page = SignaturesPage {
        signatures: Vec::new(),
        accounts: Vec::new(),
        add: FREE,
        new_id: "id-new".to_owned(),
        save: FREE,
        outcome: None,
    };
    let asked = Asked::default();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page_content(&body, &page, &actions(&asked));
    let root = body.upcast_ref::<gtk::Widget>();
    let shown = rendered_labels(root);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::settings_signatures_empty())
    );
    let buttons = descendants::<gtk::Button>(root);
    let add = button(&buttons, l10n::settings_signatures_add());
    assert_eq!(add.len(), 1);
    assert!(add[0].is_sensitive());
    add[0].emit_clicked();
    assert_eq!(*asked.borrow(), ["edit None"]);
}
