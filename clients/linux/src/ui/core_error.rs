//! What a person is shown for a core refusal.

use mailcal_bindings::{AccountCapability, AccountChangeRefusal, MailcalError, UseProblem};

use crate::l10n;

/// A refusal the core names in the person's language, since the server's reply is meant for a
/// developer and the core has logged it; anything else as the core renders it.
pub(super) fn said(error: &MailcalError) -> String {
    refusal_said(&AccountChangeRefusal::from(error))
}

/// Why a change on an account's page did not take, as [`said`] words a refusal.
pub(super) fn refusal_said(refusal: &AccountChangeRefusal) -> String {
    match refusal {
        AccountChangeRefusal::SignInRefused => l10n::signin_refused().to_owned(),
        AccountChangeRefusal::AlreadySetUp => l10n::account_already_set_up().to_owned(),
        AccountChangeRefusal::DifferentAccount => l10n::signin_different_account().to_owned(),
        AccountChangeRefusal::StaleSignIn => l10n::signin_stale().to_owned(),
        AccountChangeRefusal::UseNotConnected {
            capability,
            problem,
        } => use_not_connected(*capability, *problem).to_owned(),
        AccountChangeRefusal::Blocked { block } => {
            super::settings::account_rows::block_note(*block).to_owned()
        }
        AccountChangeRefusal::Withheld => l10n::settings_account_use_withheld().to_owned(),
        AccountChangeRefusal::NoLongerShared => {
            l10n::settings_account_shared_withdrawn().to_owned()
        }
        AccountChangeRefusal::BrowserNotOpened => l10n::account_signin_browser_failed().to_owned(),
        AccountChangeRefusal::NotFinished => l10n::account_signin_timeout().to_owned(),
        AccountChangeRefusal::Other { detail } => detail.clone(),
    }
}

/// Which use of a server of its own did not connect, and why. Only the calendar and the address
/// book move to one, so anything else is said as the calendar.
fn use_not_connected(capability: AccountCapability, problem: UseProblem) -> &'static str {
    let contacts = capability == AccountCapability::Contacts;
    match problem {
        UseProblem::Unreachable if contacts => l10n::separate_server_unreachable_contacts(),
        UseProblem::Unreachable => l10n::separate_server_unreachable_calendar(),
        UseProblem::NeedsOwnSignIn if contacts => l10n::settings_account_use_own_sign_in_contacts(),
        UseProblem::NeedsOwnSignIn => l10n::settings_account_use_own_sign_in_calendar(),
        UseProblem::SignInRefused => l10n::signin_refused(),
        UseProblem::ServerChanged if contacts => {
            l10n::settings_account_use_server_changed_contacts()
        }
        UseProblem::ServerChanged => l10n::settings_account_use_server_changed_calendar(),
        UseProblem::NoneOnServer if contacts => {
            l10n::settings_account_use_none_on_server_contacts()
        }
        UseProblem::NoneOnServer => l10n::settings_account_use_none_on_server_calendar(),
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountCapability, MailcalError, UseProblem};

    use super::said;
    use crate::l10n;

    #[test]
    fn a_refused_sign_in_is_said_in_words_and_anything_else_as_the_core_put_it() {
        let refused = MailcalError::SignInRefused {
            reason: "JMAP HTTP 401: {\"status\":401}".to_owned(),
        };
        assert_eq!(said(&refused), l10n::signin_refused());
        let twice = MailcalError::AlreadySetUp {
            account_id: "alice@example.org@imap.example.org".to_owned(),
        };
        assert_eq!(said(&twice), l10n::account_already_set_up());
        assert_eq!(
            said(&MailcalError::DifferentAccount),
            l10n::signin_different_account()
        );
        assert_eq!(said(&MailcalError::StaleSignIn), l10n::signin_stale());
        assert_eq!(
            said(&MailcalError::Connect("no route to host".to_owned())),
            "connect: no route to host"
        );
    }

    #[test]
    fn a_use_that_did_not_connect_is_named_with_why() {
        let cases = [
            (
                AccountCapability::Calendar,
                UseProblem::Unreachable,
                l10n::separate_server_unreachable_calendar(),
            ),
            (
                AccountCapability::Contacts,
                UseProblem::Unreachable,
                l10n::separate_server_unreachable_contacts(),
            ),
            (
                AccountCapability::Calendar,
                UseProblem::NeedsOwnSignIn,
                l10n::settings_account_use_own_sign_in_calendar(),
            ),
            (
                AccountCapability::Contacts,
                UseProblem::NeedsOwnSignIn,
                l10n::settings_account_use_own_sign_in_contacts(),
            ),
            (
                AccountCapability::Calendar,
                UseProblem::SignInRefused,
                l10n::signin_refused(),
            ),
            (
                AccountCapability::Contacts,
                UseProblem::SignInRefused,
                l10n::signin_refused(),
            ),
            (
                AccountCapability::Calendar,
                UseProblem::ServerChanged,
                l10n::settings_account_use_server_changed_calendar(),
            ),
            (
                AccountCapability::Contacts,
                UseProblem::ServerChanged,
                l10n::settings_account_use_server_changed_contacts(),
            ),
            // Mail never moves to a server of its own; were it named, it reads as the calendar.
            (
                AccountCapability::Mail,
                UseProblem::Unreachable,
                l10n::separate_server_unreachable_calendar(),
            ),
            (
                AccountCapability::Mail,
                UseProblem::NeedsOwnSignIn,
                l10n::settings_account_use_own_sign_in_calendar(),
            ),
            (
                AccountCapability::Mail,
                UseProblem::ServerChanged,
                l10n::settings_account_use_server_changed_calendar(),
            ),
        ];
        for (capability, problem, expected) in cases {
            let error = MailcalError::UseNotConnected {
                capability,
                problem,
            };
            assert_eq!(said(&error), expected, "{capability:?} {problem:?}");
        }
    }
}
