//! The editor's rules over a read of the page: which outcome it acts on, how its Save is drawn,
//! and how the bundle's draft is read back.

use mailcal_bindings::{
    ControlState, SignatureChangeKind, SignatureOutcome, SignatureResult, SignaturesPage,
};

use super::{outcome_for, parse_draft, save_control};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn page(outcome: Option<(u64, &str)>) -> SignaturesPage {
    SignaturesPage {
        signatures: Vec::new(),
        accounts: Vec::new(),
        add: FREE,
        new_id: "id-new".to_owned(),
        save: FREE,
        outcome: outcome.map(|(serial, id)| SignatureOutcome {
            serial,
            id: id.to_owned(),
            kind: SignatureChangeKind::Save,
            result: SignatureResult::Saved,
        }),
    }
}

#[test]
fn an_editor_acts_only_on_a_newer_outcome_for_its_own_signature() {
    // The outcome in force when it opened is an earlier save's, which must not close it.
    assert!(outcome_for(&page(Some((3, "id-work"))), 3, "id-work").is_none());
    // Another signature's save is no answer to this one.
    assert!(outcome_for(&page(Some((4, "id-home"))), 3, "id-work").is_none());
    assert!(outcome_for(&page(None), 0, "id-work").is_none());
    assert_eq!(
        outcome_for(&page(Some((4, "id-work"))), 3, "id-work").map(|outcome| outcome.serial),
        Some(4)
    );
    // A new signature is named by the page's `new_id` as it was when the editor opened.
    assert!(outcome_for(&page(Some((1, "id-new"))), 0, "id-new").is_some());
}

#[test]
fn save_is_held_while_the_core_refuses_the_name_and_runs_with_the_page() {
    assert_eq!(save_control(FREE, "Work"), FREE);
    // The core's rule, not one of this client's: a name of spaces is blank.
    assert!(!save_control(FREE, "   ").enabled);
    assert!(!save_control(FREE, "").enabled);
    let running = ControlState {
        enabled: false,
        running: true,
    };
    assert_eq!(save_control(running, "Work"), running);
}

#[test]
fn a_draft_is_read_back_by_the_keys_the_core_stores() {
    assert_eq!(
        parse_draft(r#"{"body_html":"<p>Ada</p>","body_plain":"Ada"}"#),
        Some(("<p>Ada</p>".to_owned(), "Ada".to_owned()))
    );
    // An images-only signature has no text at all; that is a signature, not a failure.
    assert_eq!(
        parse_draft(r#"{"body_html":"<img src=\"data:image/png;base64,AQ==\">"}"#),
        Some((
            "<img src=\"data:image/png;base64,AQ==\">".to_owned(),
            String::new()
        ))
    );
}

#[test]
fn an_unparsed_bundle_never_stores_an_empty_body_over_a_signature() {
    // `evaluate_javascript` on a page whose hooks do not exist yet answers with something that
    // is not the draft. Saving then has to do nothing; storing what it read would replace the
    // signature the user opened with an empty one.
    assert_eq!(parse_draft("undefined"), None);
    assert_eq!(parse_draft(""), None);
    assert_eq!(parse_draft("{}"), None);
    assert_eq!(parse_draft(r#"{"body_plain":"Ada"}"#), None);
}
