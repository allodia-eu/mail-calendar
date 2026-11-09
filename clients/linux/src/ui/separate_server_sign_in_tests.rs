//! When the form offers the sign-in at its server's own provider, and what an answer for an
//! attempt it has moved on from does.

use mailcal_bindings::{AccountCapability, AccountSeparateSignIn};

use super::{SignInEnd, SignInStage, as_connected};
use crate::{l10n, ui::separate_sign_in::tests::form};

/// The form for the calendar, at a server that offers `sign_in`.
fn offering(sign_in: Option<AccountSeparateSignIn>) -> crate::ui::separate_sign_in::SeparateForm {
    let mut form = form(vec![AccountCapability::Calendar]);
    form.section.sign_in = sign_in;
    form
}

#[test]
fn a_sign_in_starts_only_once_the_server_offers_one() {
    let mut form = offering(None);
    assert!(!form.begin_sign_in(1), "nothing starts before the answer");
    form.section.sign_in = Some(AccountSeparateSignIn::Nextcloud);
    assert!(form.begin_sign_in(1), "the same stages carry Nextcloud's");
    let mut form = offering(Some(AccountSeparateSignIn::Provider));
    form.section.connect.enabled = false;
    assert!(
        !form.begin_sign_in(1),
        "nor while a change runs on the page"
    );
}

#[test]
fn only_the_blocking_core_calls_hold_the_form() {
    let mut form = offering(Some(AccountSeparateSignIn::Provider));
    assert!(form.begin_sign_in(1));
    assert!(form.held(), "held while the core finds the provider");
    assert!(form.sign_in_prepared(1, Ok(())));
    assert_eq!(form.stage, SignInStage::Waiting(1));
    assert!(!form.held(), "the browser's wait holds nothing");
    assert!(form.sign_in_redirected(1));
    assert!(form.held(), "held while the core exchanges the code");
    assert!(!form.begin_sign_in(2), "and no press starts another then");
    assert!(
        !form.retire_sign_in(),
        "an exchange running is a core call, and finishes"
    );
    assert!(form.sign_in_ended(1));
    assert_eq!(form.stage, SignInStage::Idle);
}

#[test]
fn a_fresh_press_supersedes_the_attempt_in_the_browser() {
    let mut form = offering(Some(AccountSeparateSignIn::Provider));
    assert!(form.begin_sign_in(1));
    assert!(form.sign_in_prepared(1, Ok(())));
    assert!(form.begin_sign_in(2), "the wait does not refuse a press");

    assert!(
        !form.sign_in_prepared(1, Ok(())),
        "no browser opens for the first"
    );
    assert!(!form.sign_in_redirected(1), "its redirect is not exchanged");
    assert!(!form.sign_in_ended(1), "and its end says nothing");
    assert_eq!(form.stage, SignInStage::Starting(2));
}

#[test]
fn a_retired_attempt_opens_no_browser_and_adds_no_account() {
    let mut form = offering(Some(AccountSeparateSignIn::Provider));
    assert!(form.begin_sign_in(1));
    assert!(form.retire_sign_in(), "a begin still running is retired");
    assert!(
        !form.sign_in_prepared(1, Ok(())),
        "no browser opens after it"
    );

    assert!(form.begin_sign_in(2));
    assert!(form.sign_in_prepared(2, Ok(())));
    assert!(form.retire_sign_in(), "Cancel retires the browser's wait");
    assert_eq!(form.stage, SignInStage::Idle);
    assert!(
        !form.sign_in_redirected(2),
        "a redirect landing after it is never exchanged"
    );
    assert!(form.error.is_none(), "giving up is not a failure");
    assert!(!form.retire_sign_in(), "nothing is left to retire");
}

#[test]
fn a_provider_that_cannot_be_reached_says_so_and_offers_the_sign_in_again() {
    let mut form = offering(Some(AccountSeparateSignIn::Provider));
    assert!(form.begin_sign_in(1));
    let failed = l10n::setup_imap_signin_failed().to_owned();
    assert!(!form.sign_in_prepared(1, Err(failed.clone())));
    assert_eq!(form.stage, SignInStage::Idle);
    assert_eq!(form.error, Some(failed));
    assert!(form.begin_sign_in(2));
    assert!(
        form.error.is_none(),
        "a new press clears what the last said"
    );
}

#[test]
fn a_failed_sign_in_is_said_and_a_cancelled_one_says_nothing() {
    assert_eq!(
        as_connected(SignInEnd::Done(Err(
            l10n::signin_different_account().to_owned()
        ))),
        Some(Err(l10n::signin_different_account().to_owned()))
    );
    assert_eq!(
        as_connected(SignInEnd::Failed),
        Some(Err(l10n::setup_imap_signin_failed().to_owned()))
    );
    assert_eq!(as_connected(SignInEnd::Cancelled), None);
}

#[test]
fn a_refusal_reads_as_the_form_says_it() {
    use mailcal_bindings::MailcalError;
    assert_eq!(
        super::said(&MailcalError::Connect("no resource metadata".to_owned())),
        l10n::setup_imap_signin_failed()
    );
    assert_eq!(
        super::said(&MailcalError::DifferentAccount),
        l10n::signin_different_account()
    );
    assert_eq!(
        super::said(&MailcalError::StaleSignIn),
        l10n::signin_stale()
    );
    assert_eq!(
        super::said(&MailcalError::UseNotConnected {
            capability: AccountCapability::Contacts,
            problem: mailcal_bindings::UseProblem::NeedsOwnSignIn,
        }),
        l10n::settings_account_use_own_sign_in_contacts()
    );
}
