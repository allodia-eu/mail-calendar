//! The question a switch to the provider raises, read from the core's record.

use mailcal_bindings::{AccountCapability, BrowserSignIn, BrowserSignInStage, ControlState};

use super::question;

fn out(stage: BrowserSignInStage) -> BrowserSignIn {
    BrowserSignIn {
        attempt: 3,
        account: "alice@example.org@imap.example.org".to_owned(),
        stage,
        cancel: ControlState {
            enabled: true,
            running: false,
        },
    }
}

#[test]
fn the_question_is_asked_only_while_the_core_asks_it() {
    let asking = out(BrowserSignInStage::Asking {
        unreached: vec![AccountCapability::Calendar],
    });
    assert_eq!(
        question(Some(&asking)),
        Some((
            "alice@example.org@imap.example.org",
            3,
            [AccountCapability::Calendar].as_slice()
        ))
    );
    assert_eq!(question(Some(&out(BrowserSignInStage::Starting))), None);
    assert_eq!(question(None), None);
}
