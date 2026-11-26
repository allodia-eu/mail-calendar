//! Which action each notice about an account's sign-in or a use's outage offers.

use crate::{AccountProvider, InsecureCause, NoticeAction, UseProblem};

/// A dead sign-in's remedy: the provider's browser sign-in where there is one, the account's
/// page in Settings where a password or token has to be entered again.
pub(super) fn sign_in_remedy(provider: Option<AccountProvider>) -> NoticeAction {
    match provider {
        Some(
            AccountProvider::Microsoft
            | AccountProvider::Google
            | AccountProvider::JmapOauth
            | AccountProvider::ImapOauth,
        ) => NoticeAction::SignInAgain,
        Some(AccountProvider::Password | AccountProvider::Jmap) | None => {
            NoticeAction::OpenAccountSettings
        }
    }
}

/// A use that did not connect: dialled again by a refresh when it could not be reached or the
/// server led to plaintext, a question about the new server when it moved, and explained on the
/// account's page otherwise.
pub(super) fn outage_remedy(problem: UseProblem) -> Option<NoticeAction> {
    Some(match problem {
        UseProblem::Unreachable => NoticeAction::Retry,
        UseProblem::LedToPlaintext => insecure_remedy(InsecureCause::LedToPlaintext),
        UseProblem::PlaintextAddress => insecure_remedy(InsecureCause::PlaintextAddress),
        UseProblem::ServerChanged => NoticeAction::Review,
        UseProblem::NeedsOwnSignIn | UseProblem::SignInRefused | UseProblem::NoneOnServer => {
            NoticeAction::OpenAccountSettings
        }
    })
}

/// A server reached only over plaintext: the account's page, where a plaintext address is given
/// a secure one; a retry where the server led there, since only its operator can change that.
pub(super) fn insecure_remedy(cause: InsecureCause) -> NoticeAction {
    match cause {
        InsecureCause::PlaintextAddress => NoticeAction::OpenAccountSettings,
        InsecureCause::LedToPlaintext => NoticeAction::Retry,
    }
}
