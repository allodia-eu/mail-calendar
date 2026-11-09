//! An account's Sign-in section on its page (`docs/accounts.md` rule 17): which ways in its
//! server takes, asked in the background when the page opens and kept so the page is drawn on
//! every signal without asking again, and the section decided from that answer. No client keeps a
//! copy.

use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    AccountEntry, AccountKind, AccountPasswordField, AccountSecretKind, AccountSignIn,
    AccountSignInLine, AccountSignInOptions, AccountSignInRoute, CapabilityState, ControlState,
    SignInMethod,
    account_work::{Hold, PageWork},
};

#[derive(Debug, Default)]
struct Answer {
    options: Option<AccountSignInOptions>,
    /// The ask out now, if any: one at a time.
    asking: Option<u64>,
}

/// What each account's server said it takes.
#[derive(Debug, Default)]
pub(crate) struct SignInAnswers {
    answers: Mutex<HashMap<String, Answer>>,
    tickets: AtomicU64,
}

impl SignInAnswers {
    fn answers(&self) -> std::sync::MutexGuard<'_, HashMap<String, Answer>> {
        self.answers.lock().expect("sign-in answers mutex poisoned")
    }

    /// A ticket to ask `account`'s server, when no ask is out and it has not answered, or is asked
    /// `again` (its page opened). What it answered before is kept until the new answer lands.
    pub(crate) fn ask(&self, account: &str, again: bool) -> Option<u64> {
        let mut answers = self.answers();
        let answer = answers.entry(account.to_owned()).or_default();
        if answer.asking.is_some() || (answer.options.is_some() && !again) {
            return None;
        }
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed) + 1;
        answer.asking = Some(ticket);
        Some(ticket)
    }

    /// The server answered the ask `ticket`. An ask the account's sign-in changed under is
    /// dropped: it was about the way in the account no longer has.
    pub(crate) fn answered(&self, account: &str, ticket: u64, options: AccountSignInOptions) {
        if let Some(answer) = self.answers().get_mut(account)
            && answer.asking == Some(ticket)
        {
            answer.asking = None;
            answer.options = Some(options);
        }
    }

    /// What `account`'s server last answered.
    pub(crate) fn options(&self, account: &str) -> Option<AccountSignInOptions> {
        self.answers()
            .get(account)
            .and_then(|answer| answer.options.clone())
    }

    /// Drops what is kept for `account`, whose way in changed or which is gone, and any ask out.
    pub(crate) fn forget(&self, account: &str) {
        self.answers().remove(account);
    }
}

/// `entry`'s Sign-in section, from what its server said it takes (`None` until it has answered)
/// and whether it refused the stored sign-in (`refused`), held as `hold` says. `None` for an
/// account with no section: a calendar and contacts account that is not at a Nextcloud server.
pub(crate) fn section(
    entry: &AccountEntry,
    options: Option<&AccountSignInOptions>,
    refused: bool,
    hold: &Hold,
) -> Option<AccountSignIn> {
    let control = ControlState {
        enabled: !hold.busy,
        running: false,
    };
    let typing = hold.own.contains(&PageWork::Password);
    let save = ControlState {
        enabled: !hold.busy,
        running: typing,
    };
    let (line, routes, password) = match entry.kind {
        AccountKind::Microsoft | AccountKind::Google => {
            let line = if refused {
                AccountSignInLine::Refused
            } else if entry
                .uses
                .iter()
                .any(|use_| use_.state == CapabilityState::NeedsPermission)
            {
                AccountSignInLine::NeedsPermission
            } else {
                AccountSignInLine::AtProvider
            };
            (line, vec![AccountSignInRoute::SignInAgain], None)
        }
        AccountKind::Dav => {
            let offered = options.is_some_and(|options| options.nextcloud);
            if !entry.nextcloud && !offered {
                return None;
            }
            let route = match (offered, entry.nextcloud) {
                (false, _) => None,
                (true, true) => Some(AccountSignInRoute::SignInAgain),
                (true, false) => Some(AccountSignInRoute::ToNextcloud),
            };
            let line = match (entry.nextcloud, refused) {
                (true, true) => AccountSignInLine::Refused,
                (true, false) => AccountSignInLine::Nextcloud,
                (false, _) => AccountSignInLine::Password,
            };
            (line, route.into_iter().collect(), None)
        }
        AccountKind::Imap | AccountKind::Jmap => {
            let options = options
                .cloned()
                .unwrap_or_else(crate::account_sign_in::no_options);
            match entry.sign_in {
                SignInMethod::Provider => {
                    let routes = if options.sign_in {
                        vec![AccountSignInRoute::SignInAgain]
                    } else {
                        Vec::new()
                    };
                    let password = options.password.then_some(AccountPasswordField {
                        kind: AccountSecretKind::Password,
                        replaces: false,
                        save,
                    });
                    let line = if refused && !routes.is_empty() {
                        AccountSignInLine::Refused
                    } else {
                        AccountSignInLine::Provider
                    };
                    (line, routes, password)
                }
                SignInMethod::Password => {
                    let routes = if options.sign_in {
                        vec![AccountSignInRoute::ToProvider {
                            label: options.provider_label.filter(|label| !label.is_empty()),
                        }]
                    } else {
                        Vec::new()
                    };
                    // An account whose servers its page edits takes its new password there.
                    let replaces = refused && entry.endpoints.is_none();
                    let password = replaces.then_some(AccountPasswordField {
                        kind: if entry.kind == AccountKind::Jmap {
                            AccountSecretKind::PasswordOrToken
                        } else {
                            AccountSecretKind::Password
                        },
                        replaces: true,
                        save,
                    });
                    let line = if replaces {
                        AccountSignInLine::Refused
                    } else {
                        AccountSignInLine::Password
                    };
                    (line, routes, password)
                }
            }
        }
    };
    let switches = routes
        .iter()
        .any(|route| matches!(route, AccountSignInRoute::ToProvider { .. }))
        || password.is_some_and(|field| !field.replaces);
    Some(AccountSignIn {
        line,
        routes,
        control,
        password,
        switches,
    })
}

#[cfg(test)]
#[path = "account_page_sign_in_tests.rs"]
mod tests;
