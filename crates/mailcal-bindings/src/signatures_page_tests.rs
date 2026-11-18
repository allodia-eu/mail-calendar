//! Settings → Signatures as it is put together: free while nothing runs, and while a change runs
//! or waits every control held and the one that started it running.

use mailcal_viewmodel::{
    AccountSignatureRow, SignatureChangeKind as AppKind, SignatureOutcome as AppOutcome,
    SignatureRefusal as AppRefusal, SignatureResult as AppResult, SignatureRow, SignatureWork,
    SignaturesView,
};

use super::page;
use crate::{ControlState, SignatureRefusal, SignatureResult};

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

fn view(work: Vec<SignatureWork>) -> SignaturesView {
    SignaturesView {
        library: vec![
            SignatureRow {
                id: "work".to_owned(),
                name: "Work".to_owned(),
            },
            SignatureRow {
                id: "home".to_owned(),
                name: "Home".to_owned(),
            },
        ],
        accounts: vec![AccountSignatureRow {
            account_id: "acct".to_owned(),
            email: "me@example.org".to_owned(),
            new_message: Some("work".to_owned()),
            reply_forward: None,
        }],
        new_id: "fresh".to_owned(),
        work,
        outcome: None,
    }
}

fn work(id: &str, kind: AppKind) -> SignatureWork {
    SignatureWork {
        id: id.to_owned(),
        kind,
    }
}

#[test]
fn every_control_is_free_while_nothing_runs() {
    let page = page(view(Vec::new()));
    assert_eq!(page.add, FREE);
    assert_eq!(page.save, FREE);
    assert_eq!(page.new_id, "fresh");
    assert!(
        page.signatures
            .iter()
            .all(|entry| entry.edit == FREE && entry.delete == FREE)
    );
    let account = &page.accounts[0];
    assert_eq!(account.new_message.selected.as_deref(), Some("work"));
    assert_eq!(account.new_message.control, FREE);
    assert_eq!(account.reply_forward.selected, None);
}

/// A save running holds the whole page and runs Save, whether it is the save of a new signature
/// or of one in the library.
#[test]
fn a_save_runs_its_button_and_holds_the_rest() {
    let page = page(view(vec![work("fresh", AppKind::Save)]));
    assert_eq!(page.save, RUNNING);
    assert_eq!(page.add, HELD);
    assert!(
        page.signatures
            .iter()
            .all(|entry| entry.edit == HELD && entry.delete == HELD)
    );
    assert_eq!(page.accounts[0].new_message.control, HELD);
    assert_eq!(page.accounts[0].reply_forward.control, HELD);
}

/// A deletion runs the row it deletes, and one waiting behind a save runs its row too.
#[test]
fn a_deletion_runs_its_own_row_only() {
    let page = page(view(vec![
        work("home", AppKind::Save),
        work("work", AppKind::Delete),
    ]));
    let delete = |id: &str| {
        page.signatures
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .delete
    };
    assert_eq!(delete("work"), RUNNING);
    assert_eq!(delete("home"), HELD);
    assert_eq!(page.save, RUNNING);
}

#[test]
fn the_outcome_carries_its_serial_and_reason() {
    let mut view = view(Vec::new());
    view.outcome = Some(AppOutcome {
        serial: 7,
        id: "fresh".to_owned(),
        kind: AppKind::Save,
        result: AppResult::Refused {
            reason: AppRefusal::BlankName,
        },
    });
    let outcome = page(view).outcome.expect("an outcome");
    assert_eq!(outcome.serial, 7);
    assert_eq!(outcome.id, "fresh");
    assert_eq!(
        outcome.result,
        SignatureResult::Refused {
            reason: SignatureRefusal::BlankName
        }
    );
}
