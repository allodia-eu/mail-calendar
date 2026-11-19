//! Which account a new message is written from when nothing names one, and Settings → Composing's
//! record of the stored choice behind it.
//!
//! In order: the **selected** account (its mailbox is showing, so it scopes the choice), else the
//! persisted **default send account**, else the first account. Only accounts used for mail are
//! candidates (`docs/accounts.md`): one kept for its calendar or contacts alone sends nothing.
//! The stored default lives in the shared preferences file; it may name an account since removed
//! or no longer used for mail, which reads as the first one that is.

use engine_api::{AccountId, Provider};
use mailcal_viewmodel::{SendAccountOption, SendAccountSetting};

use crate::{App, Surface, preferences_file::PreferencesFile};

/// The loaded default send account (an account id) and where to persist it.
pub(crate) struct SendSettingsState {
    account: Option<String>,
    prefs: PreferencesFile,
}

impl SendSettingsState {
    /// Loads the persisted default from the preferences file (`None` when absent or unreadable).
    pub(crate) fn new(prefs: PreferencesFile) -> Self {
        Self {
            account: prefs.load().default_send_account,
            prefs,
        }
    }

    fn get(&self) -> Option<String> {
        self.account.clone()
    }

    /// Stores `account` and persists it. A blank id is normalised to `None` ("no default; derive
    /// it") rather than stored as an empty string.
    fn set(&mut self, account: Option<String>) {
        self.account = account.filter(|id| !id.trim().is_empty());
        let account = self.account.clone();
        self.prefs
            .edit(move |prefs| prefs.default_send_account.clone_from(&account));
    }
}

/// The sender the stored default comes to among `senders`: the one it names, else the first.
fn default_among<'a>(
    senders: &'a [(AccountId, String)],
    stored: Option<&str>,
) -> Option<&'a (AccountId, String)> {
    stored
        .and_then(|stored| senders.iter().find(|(id, _)| id.as_str() == stored))
        .or_else(|| senders.first())
}

impl<P: Provider> App<P> {
    /// The account a new message is written from when nothing names one. Read from memory, so
    /// it never waits on the account list.
    pub(crate) fn compose_account(&self) -> Option<AccountId> {
        self.sender_or_default(None)
    }

    /// `account` when it is used for mail, else [`App::compose_account`]'s answer: what a
    /// composer opens on when a host or an assistant may name an account.
    pub(crate) fn sender_or_default(&self, account: Option<AccountId>) -> Option<AccountId> {
        let senders = self.accounts.senders();
        let sends = |account: &AccountId| senders.iter().any(|(id, _)| id == account);
        let selected = self
            .scope
            .lock()
            .expect("scope mutex poisoned")
            .account()
            .cloned();
        if let Some(account) = account.filter(sends).or_else(|| selected.filter(sends)) {
            return Some(account);
        }
        let stored = self.stored_send_account();
        default_among(&senders, stored.as_deref()).map(|(id, _)| id.clone())
    }

    /// Whether `account` is used for mail, so a message can come from it.
    pub(crate) fn is_sender(&self, account: &AccountId) -> bool {
        self.accounts.senders().iter().any(|(id, _)| id == account)
    }

    /// Settings → Composing's default send account (pulled after a [`Surface::Settings`]
    /// signal, which a change to the accounts raises too). Read from memory, so a UI thread may
    /// ask it.
    #[must_use]
    pub fn send_account_setting(&self) -> SendAccountSetting {
        let senders = self.accounts.senders();
        let stored = self.stored_send_account();
        match senders.as_slice() {
            [] => SendAccountSetting::NoAccount,
            [(_, email)] => SendAccountSetting::Only {
                email: email.clone(),
            },
            _ => SendAccountSetting::Choose {
                selected: default_among(&senders, stored.as_deref())
                    .map(|(id, _)| id.as_str().to_owned())
                    .unwrap_or_default(),
                accounts: senders
                    .iter()
                    .map(|(id, email)| SendAccountOption {
                        id: id.as_str().to_owned(),
                        email: email.clone(),
                    })
                    .collect(),
            },
        }
    }

    fn stored_send_account(&self) -> Option<String> {
        self.send_settings
            .lock()
            .expect("send-settings mutex poisoned")
            .get()
    }

    /// Sets and persists the default send account (`None` clears it, restoring "the first
    /// account used for mail"), then signals [`Surface::Settings`] so the host re-pulls.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly (this one just locks,
    // sets, and signals).
    #[allow(clippy::unused_async)]
    pub async fn set_default_send_account(&self, account: Option<String>) {
        self.send_settings
            .lock()
            .expect("send-settings mutex poisoned")
            .set(account);
        self.observer.surface_changed(Surface::Settings);
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{QuoteStyle, load_preferences};

    use super::SendSettingsState;
    use crate::preferences_file::{PreferencesFile, ScratchPreferences};

    #[test]
    fn set_persists_and_reloads_without_clobbering_sibling_preferences() {
        // Seed a sibling preference the send-account write must preserve.
        let scratch = ScratchPreferences::seeded(&mailcal_account::Preferences {
            quote_style: QuoteStyle::LineAndHeader,
            ..Default::default()
        });

        // No default until one is chosen, then a set persists it.
        let mut state = SendSettingsState::new(scratch.open());
        assert_eq!(state.get(), None);
        state.set(Some("acct-2".to_owned()));

        // A fresh state reads back the persisted choice, and the sibling pref survived.
        assert_eq!(
            SendSettingsState::new(scratch.open()).get().as_deref(),
            Some("acct-2")
        );
        assert_eq!(
            load_preferences(scratch.path()).quote_style,
            QuoteStyle::LineAndHeader
        );

        // Clearing it restores "derive the account", and persists that too.
        state.set(None);
        assert_eq!(SendSettingsState::new(scratch.open()).get(), None);
    }

    #[test]
    fn a_blank_account_id_is_stored_as_no_default() {
        // A host that hands back an empty selection means "no default", not an account whose id
        // is the empty string (which could never match a configured account).
        let mut state = SendSettingsState::new(PreferencesFile::default());
        state.set(Some("   ".to_owned()));
        assert_eq!(state.get(), None);
    }
}
