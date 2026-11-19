//! The own endpoint's form as the core's record draws it (`docs/ai.md` rules 6 and 7). Called
//! from the crate's single `gtk::init` test.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{
    AiEndpointAddressGap, AiEndpointChangeKind, AiEndpointForm, AiEndpointGaps, AiEndpointIntent,
    AiEndpointOutcome, AiEndpointResult, ControlState, JurisdictionClass,
};

use super::Form;
use crate::{
    l10n,
    ui::{controls::tests::spins, mailbox::tests::rendered_labels},
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};

fn record() -> AiEndpointForm {
    AiEndpointForm {
        address: "http://localhost:28434/v1".to_owned(),
        model: "mock".to_owned(),
        declared: Some(JurisdictionClass::EuNative),
        key_kept: true,
        gaps: AiEndpointGaps {
            address: None,
            model_missing: false,
            declaration: None,
        },
        save: FREE,
        remove: FREE,
        outcome: None,
    }
}

/// The core's rule as a test can state it: no address is missing, anything else is complete.
fn form(sent: &Rc<RefCell<Vec<String>>>) -> Rc<Form> {
    let sent = Rc::clone(sent);
    Form::new(
        Rc::new(|address: String, model: String, declared| AiEndpointGaps {
            address: address.is_empty().then_some(AiEndpointAddressGap::Missing),
            model_missing: model.is_empty(),
            declaration: declared
                .is_none()
                .then_some(mailcal_bindings::AiEndpointDeclarationGap::Missing),
        }),
        Rc::new(move |intent| {
            sent.borrow_mut().push(match intent {
                AiEndpointIntent::Save {
                    address,
                    key,
                    declared,
                    ..
                } => format!("save {address} {key:?} {declared:?}"),
                AiEndpointIntent::Remove => "remove".to_owned(),
            });
        }),
    )
}

fn button(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>()
            && rendered_labels(button.upcast_ref())
                .iter()
                .any(|text| text == label)
        {
            return Some(button.clone());
        }
        if let Some(found) = button(&widget, label) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

pub(crate) fn the_form_is_absent_while_the_core_offers_none() {
    let form = form(&Rc::default());
    form.draw(None);
    assert!(!form.root.is_visible());
    form.draw(Some(record()));
    assert!(form.root.is_visible());
}

/// The fields are the record's, a kept key is said and never drawn back, and an empty key field
/// sends no key.
pub(crate) fn save_sends_the_fields_as_typed_and_no_key_for_an_empty_field() {
    let sent = Rc::default();
    let form = form(&sent);
    form.draw(Some(record()));
    assert_eq!(form.address.text(), "http://localhost:28434/v1");
    assert_eq!(form.key.text(), "", "a key is never drawn back");
    assert_eq!(form.key_note.text(), l10n::ai_endpoint_key_stored());
    let root = form.root.upcast_ref::<gtk::Widget>();
    button(root, l10n::ai_endpoint_save())
        .expect("Save")
        .emit_clicked();
    assert_eq!(
        *sent.borrow(),
        ["save http://localhost:28434/v1 None Some(EuNative)"]
    );
}

/// A gap is said under its field as the person types, and Save is held while one is named.
pub(crate) fn a_gap_is_said_under_its_field_and_holds_save() {
    let form = form(&Rc::default());
    form.draw(Some(record()));
    form.model.set_text("");
    assert!(form.model_note.is_visible());
    assert_eq!(
        form.model_note.text(),
        l10n::ai_endpoint_gap_model_missing()
    );
    let root = form.root.upcast_ref::<gtk::Widget>();
    let save = button(root, l10n::ai_endpoint_save()).expect("Save");
    assert!(!save.is_sensitive());
    form.model.set_text("mock");
    assert!(!form.model_note.is_visible());
    let save = button(root, l10n::ai_endpoint_save()).expect("Save");
    assert!(save.is_sensitive());
}

/// While a save runs, Save spins, Remove and every field are held, and typing is not drawn over;
/// what the keystore refused is said on the form.
pub(crate) fn a_running_save_holds_the_form_and_a_refused_key_is_said() {
    let form = form(&Rc::default());
    form.draw(Some(record()));
    form.draw(Some(AiEndpointForm {
        save: ControlState {
            enabled: false,
            running: true,
        },
        remove: HELD,
        ..record()
    }));
    let root = form.root.upcast_ref::<gtk::Widget>();
    let save = button(root, l10n::ai_endpoint_save()).expect("Save");
    assert!(spins(&save) && !save.is_sensitive());
    assert!(
        button(root, l10n::ai_endpoint_remove()).is_none(),
        "nothing stored, no Remove"
    );
    assert!(!form.address.is_sensitive() && !form.model.is_sensitive());
    form.draw(Some(AiEndpointForm {
        outcome: Some(AiEndpointOutcome {
            serial: 3,
            kind: AiEndpointChangeKind::Save,
            result: AiEndpointResult::KeyNotStored,
        }),
        ..record()
    }));
    assert!(form.failed.is_visible());
    assert_eq!(form.failed.text(), l10n::ai_endpoint_key_not_stored());
    assert!(form.address.is_sensitive());
}
