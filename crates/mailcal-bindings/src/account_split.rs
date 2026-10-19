//! Moving a standards account's calendar and contacts to an account of their own, when the
//! account's sign-in does not reach their server (`docs/accounts.md` rule 18).
//!
//! An account is one sign-in, so a calendar that takes a password beside a mailbox that signs in
//! through its provider is a second account, linked to the first. The calendar account is added,
//! and has connected, before the mailbox changes, so a refused password leaves both as they were.

use engine_api::AccountId;
use mailcal_account::{AccountConfig, Capability};

use crate::{
    AccountCapability, MailcalApp, MailcalError, account_registry::SignInConfig,
    account_sign_in::Replacement, app_accounts_uses::domain_of,
};

/// What becomes of the calendar and contacts a new sign-in does not reach, chosen before the
/// browser opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum UnreachedUses {
    /// They keep the password they connect with now, as an account of their own linked to this
    /// one.
    KeepOnPassword,
    /// The account stops being used for them, and what the device holds of them is deleted.
    StopUsing,
}

#[uniffi::export]
impl MailcalApp {
    /// Moves `uses` (calendar, contacts or both) of `account_id` to an account of their own at the
    /// same servers, signing in as `username` with `password`, and links the two: what a person
    /// is asked for when the account's own sign-in does not reach that server.
    ///
    /// Returns the new account's id. **Blocking** (two connects); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::SignInRefused`] when the server refuses the password,
    /// [`MailcalError::Connect`] when it cannot be reached, and [`MailcalError::Config`] for an
    /// empty password, an account that is not a standards one, or `uses` naming neither calendar
    /// nor contacts or a server the account lacks. Both accounts are unchanged on every failure.
    pub fn add_linked_calendar_account(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
        username: String,
        password: String,
    ) -> Result<String, MailcalError> {
        if password.is_empty() {
            return Err(MailcalError::Config("the password is empty".to_owned()));
        }
        let config = match self.switchable(&account_id)? {
            SignInConfig::Imap(config) if config.imap.is_some() => config,
            _ => {
                return Err(MailcalError::Config(
                    "only a standards mailbox keeps its calendar on a server of its own".to_owned(),
                ));
            }
        };
        let uses: Vec<Capability> = uses.into_iter().map(Capability::from).collect();
        let calendar = config
            .calendar_account_for(&uses)
            .ok_or_else(nothing_to_move)?
            .with_dav_login(username.trim(), &password);
        self.split_out(&account_id, calendar, *config, &uses)
    }
}

impl MailcalApp {
    /// Installs `signed`, the account `before` was with the grant a sign-in just issued, doing
    /// with the calendar and contacts the grant does not reach what the person chose.
    ///
    /// Only a switch from a password asks: an account that already signs in through its
    /// provider has no other credential to keep them on.
    pub(crate) fn install_signed_in(
        &self,
        account_id: &str,
        before: &AccountConfig,
        signed: AccountConfig,
        choice: Option<UnreachedUses>,
    ) -> Result<(), MailcalError> {
        let lost = if before.is_oauth() {
            Vec::new()
        } else {
            signed.uses_its_grant_does_not_reach()
        };
        if lost.is_empty() {
            return self.install_credential(account_id, Replacement::Imap(signed), &[]);
        }
        match choice {
            None => Err(MailcalError::Config(
                "this sign-in does not reach the account's calendar or contacts, and nothing was \
                 chosen for them"
                    .to_owned(),
            )),
            Some(UnreachedUses::StopUsing) => {
                log::info!(
                    "sign-in: [{}] stops using what the new sign-in does not reach",
                    mailcal_account::account_log_handle(account_id),
                );
                self.install_credential(
                    account_id,
                    Replacement::Imap(signed.without_uses(&lost, None)),
                    &domains(&lost),
                )
            }
            Some(UnreachedUses::KeepOnPassword) => {
                let calendar = before
                    .calendar_account_for(&lost)
                    .ok_or_else(nothing_to_move)?;
                self.split_out(account_id, calendar, signed, &lost)
                    .map(|_| ())
            }
        }
    }

    /// Adds `calendar`, then installs `mailbox` without `uses` and linked to it, removing the
    /// calendar account again when the mailbox does not install.
    pub(crate) fn split_out(
        &self,
        account_id: &str,
        calendar: AccountConfig,
        mailbox: AccountConfig,
        uses: &[Capability],
    ) -> Result<String, MailcalError> {
        let toml = calendar
            .to_toml()
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        // The same servers and login already set up as an account of their own is the account to
        // link to, and one this move did not add is not one it may remove.
        let (moved_to, added) = match self.add_account(toml) {
            Ok(row) => (row.id, true),
            Err(MailcalError::AlreadySetUp {
                account_id: existing,
            }) if existing != account_id => (existing, false),
            // An account used for nothing but these fails as a whole when none of them connects,
            // and it is still these that did not.
            Err(MailcalError::Connect(_)) => {
                return Err(MailcalError::UseNotConnected {
                    capability: uses.first().copied().unwrap_or(Capability::Calendar).into(),
                    problem: crate::UseProblem::Unreachable,
                });
            }
            Err(error) => return Err(error),
        };
        if added && let Some(error) = self.failed_use(&moved_to, uses) {
            self.undo_split(&moved_to);
            return Err(error);
        }
        let id = AccountId::try_from(moved_to.as_str())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let mailbox = mailbox.without_uses(uses, Some(&id));
        if let Err(error) =
            self.install_credential(account_id, Replacement::Imap(mailbox), &domains(uses))
        {
            if added {
                self.undo_split(&moved_to);
            }
            return Err(error);
        }
        log::info!(
            "accounts: [{}] keeps its calendar and contacts on [{}], an account of their own",
            mailcal_account::account_log_handle(account_id),
            mailcal_account::account_log_handle(&moved_to),
        );
        Ok(moved_to)
    }

    /// Why the calendar account just added did not connect one of `uses`, as an error.
    fn failed_use(&self, account_id: &str, uses: &[Capability]) -> Option<MailcalError> {
        let id = AccountId::try_from(account_id).ok()?;
        let problems = self.app.use_problems(&id);
        uses.iter()
            .filter_map(|&capability| {
                let problem = match capability {
                    Capability::Calendar => problems.calendar,
                    Capability::Contacts => problems.contacts,
                    _ => None,
                }?;
                Some(match problem {
                    mailcal_app::UseProblem::SignInRefused => MailcalError::SignInRefused {
                        reason: "the calendar or contacts server refused the password".to_owned(),
                    },
                    other => MailcalError::UseNotConnected {
                        capability: capability.into(),
                        problem: other.into(),
                    },
                })
            })
            .next()
    }

    fn undo_split(&self, moved_to: &str) {
        if let Err(error) = self.remove_account(moved_to.to_owned()) {
            log::warn!(
                "accounts: [{}] could not be removed after the move failed: {error}",
                mailcal_account::account_log_handle(moved_to),
            );
        }
    }
}

fn domains(uses: &[Capability]) -> Vec<engine_api::SearchDomain> {
    uses.iter()
        .map(|&capability| domain_of(capability))
        .collect()
}

fn nothing_to_move() -> MailcalError {
    MailcalError::Config("the account has no calendar or contacts server to move".to_owned())
}

#[cfg(test)]
#[path = "account_split_tests.rs"]
mod tests;
