//! The traffic of Settings → Accounts' account pages: which page is open, and the changes made on
//! one. Each change is a core call that may touch the keyring or delete local data, so it runs
//! off the main thread and redraws the page when it is done.

use mailcal_bindings::{AccountCapability, CapabilityChange, LinkSlot};

use super::{AppInput, AppModel};
use crate::l10n;

/// One input rather than several, so the dispatch has a single arm: they are one page's traffic
/// and each ends in the same redraw.
#[derive(Debug)]
pub(crate) enum AccountsInput {
    /// Open an account's page, or go back to the list.
    Show(Option<String>),
    /// Switch one of an account's uses on or off.
    SetUse {
        account: String,
        capability: AccountCapability,
        on: bool,
    },
    /// Link an account to another for one slot, or clear it.
    SetLink {
        account: String,
        slot: LinkSlot,
        target: Option<String>,
    },
    /// A change finished, with what the page should say about it, if anything.
    Changed(Option<String>),
}

impl AppModel {
    /// One piece of the account pages' traffic.
    pub(super) fn accounts_input(&mut self, input: AccountsInput, sender: relm4::Sender<AppInput>) {
        match input {
            AccountsInput::Show(account) => {
                self.settings.account = account;
                self.settings.account_notice = None;
                self.settings.refresh_in_place();
            }
            AccountsInput::SetUse {
                account,
                capability,
                on,
            } => self.set_account_use(account, capability, on, sender),
            AccountsInput::SetLink {
                account,
                slot,
                target,
            } => self.set_account_link(account, slot, target, sender),
            AccountsInput::Changed(notice) => {
                self.settings.account_notice = notice;
                self.settings.refresh_in_place();
            }
        }
    }

    fn set_account_use(
        &self,
        account: String,
        capability: AccountCapability,
        on: bool,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let address = app
            .accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == account)
            .map_or_else(|| account.clone(), |entry| entry.address);
        // Blocking: switching a use off deletes what the device holds of it.
        std::thread::spawn(move || {
            let notice = match app.set_account_capability(account, capability, on) {
                Ok(CapabilityChange::Applied) => None,
                Ok(CapabilityChange::NeedsConsent) => {
                    Some(l10n::settings_account_use_needs_consent(&address))
                }
                Ok(CapabilityChange::NeedsEndpoint) => {
                    Some(l10n::settings_account_use_needs_endpoint().to_owned())
                }
                Err(error) => Some(l10n::settings_account_change_failed(&error.to_string())),
            };
            sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
        });
    }

    fn set_account_link(
        &self,
        account: String,
        slot: LinkSlot,
        target: Option<String>,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        // The link is stored through the host's keyring, which may block.
        std::thread::spawn(move || {
            let notice = app
                .set_account_link(account, slot, target)
                .err()
                .map(|error| l10n::settings_account_change_failed(&error.to_string()));
            sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
        });
    }
}
