//! An account's Sign-in section on its page (`docs/accounts.md` rule 17): which ways in its
//! server takes, asked in the background when the page opens and kept so the page is drawn on
//! every signal without asking again, and the section decided from that answer. No client keeps a
//! copy.

use crate::{
    AccountEntry, AccountKind, AccountPasswordField, AccountSecretKind, AccountSignIn,
    AccountSignInLine, AccountSignInOptions, AccountSignInRoute, CapabilityState, ControlState,
    SignInMethod,
    account_work::{Hold, PageWork},
};

/// What each account's server said it takes.
pub(crate) type SignInAnswers = crate::account_answers::Answers<AccountSignInOptions>;

/// `entry`'s Sign-in section, from what its server said it takes (`None` until it has answered)
/// and whether it refused the stored sign-in (`refused`), held as `hold` says. `edits_servers`:
/// its page has a servers form, which takes a new password itself. `None` for an account with no
/// section: a calendar and contacts account that is not at a Nextcloud server.
pub(crate) fn section(
    entry: &AccountEntry,
    options: Option<&AccountSignInOptions>,
    refused: bool,
    edits_servers: bool,
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
                    // A password in place of the refused grant is a remedy as much as a sign-in.
                    let line = if refused && (!routes.is_empty() || password.is_some()) {
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
                    let replaces = refused && !edits_servers;
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
