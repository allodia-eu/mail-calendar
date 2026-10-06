//! The traffic of Settings → Accounts' account pages: which page is open, and the changes made on
//! one. Each change is a core call that may touch the keyring or delete local data, so it runs
//! off the main thread and redraws the page when it is done.

use mailcal_bindings::{AccountCapability, CapabilityChange, LinkSlot, MailcalError};

use super::{
    AppInput, AppModel,
    account_consent::{ConsentFinished, ConsentFrom},
    settings::notice::Notice,
};
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
    /// Sign the account in again at its provider, asking for what it is used for plus `adding`.
    SignInAgain {
        account: String,
        adding: Vec<AccountCapability>,
    },
    /// A sign-in asked for here or by a banner ended.
    Consented(ConsentFinished),
    /// A change finished, with what the page should say about it, if anything.
    Changed(Option<Notice>),
}

/// What the page does once switching a use has answered. A use the provider has not granted is
/// asked for straight away: the person switching it on is the person who would be asked.
fn use_changed(
    result: Result<CapabilityChange, MailcalError>,
    account: String,
    capability: AccountCapability,
) -> AccountsInput {
    match result {
        Ok(CapabilityChange::Applied) => AccountsInput::Changed(None),
        Ok(CapabilityChange::NeedsConsent) => AccountsInput::SignInAgain {
            account,
            adding: vec![capability],
        },
        Ok(CapabilityChange::NeedsEndpoint) => AccountsInput::Changed(Some(change_failed(
            l10n::settings_account_use_needs_endpoint().to_owned(),
        ))),
        Err(error) => AccountsInput::Changed(Some(change_failed(error.to_string()))),
    }
}

/// A change the account's page could not make, and why.
fn change_failed(detail: String) -> Notice {
    Notice::Error {
        title: l10n::settings_account_change_failed_title().to_owned(),
        detail,
    }
}

impl AppModel {
    /// One piece of the account pages' traffic.
    pub(super) fn accounts_input(&mut self, input: AccountsInput, sender: relm4::Sender<AppInput>) {
        match input {
            AccountsInput::Show(account) => {
                self.settings.account = account;
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
            AccountsInput::SignInAgain { account, adding } => {
                self.start_account_consent(account, adding, ConsentFrom::Settings, sender);
            }
            AccountsInput::Consented(finished) => self.account_consent_finished(&finished),
            AccountsInput::Changed(Some(notice)) => self.settings.notify(notice),
            AccountsInput::Changed(None) => self.settings.refresh_in_place(),
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
        // Blocking: switching a use off deletes what the device holds of it.
        std::thread::spawn(move || {
            let result = app.set_account_capability(account.clone(), capability, on);
            sender.emit(AppInput::Accounts(use_changed(result, account, capability)));
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
                .map(|error| change_failed(error.to_string()));
            sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
        });
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountCapability, CapabilityChange};

    use super::{AccountsInput, use_changed};

    #[test]
    fn switching_on_a_use_the_provider_withholds_asks_for_it() {
        let input = use_changed(
            Ok(CapabilityChange::NeedsConsent),
            "a".to_owned(),
            AccountCapability::Contacts,
        );
        assert!(matches!(
            input,
            AccountsInput::SignInAgain { account, adding }
                if account == "a" && adding == [AccountCapability::Contacts]
        ));
        assert!(matches!(
            use_changed(
                Ok(CapabilityChange::Applied),
                "a".to_owned(),
                AccountCapability::Contacts
            ),
            AccountsInput::Changed(None)
        ));
    }
}
