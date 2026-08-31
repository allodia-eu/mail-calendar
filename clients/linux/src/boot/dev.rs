//! The debug-only halves of a boot: the fixture credential stores, and carrying a real Allodia
//! sign-in across into a fixture launch.
//!
//! Split from [`super`] to keep both files inside the 500-line limit. Everything here is
//! `#[cfg(debug_assertions)]`, so a release build compiles none of it.

use std::sync::Arc;

use mailcal_bindings::{AccountCredentialStore, CredentialStoreError};

use crate::secrets::{SecretSink, SecretStore};

pub(super) fn dev_secrets(namespace: &str) -> Option<Arc<SecretStore>> {
    match SecretStore::open_dev(namespace) {
        Ok(store) => Some(Arc::new(store)),
        Err(error) => {
            log::warn!(
                "this debug build could not open its own secure store, so a sign-in made here \
                 will not be kept: {error}"
            );
            None
        }
    }
}

/// The answer when this build has no keyring to write to at all.
///
/// The core requires a store, and requiring it is the point: a rotated refresh token that reaches
/// none leaves the persisted credential behind the server's, and a replay-detecting authorization
/// server answers the superseded token by revoking the grant (`docs/provider-oauth.md` rule 5). So
/// both methods **refuse** rather than quietly succeeding; an error the core can act on, instead
/// of a credential dropped in silence.
///
/// Only a debug launch whose Secret Service would not open reaches this now; a fixture launch that
/// has one writes through it, on a namespace of its own.
struct NoSecretStore;

impl NoSecretStore {
    /// The one answer both methods give, so the reason is written once.
    ///
    /// It names no account: the id is `<address>@<host>`, so logging it would put a user's address
    /// in a file meant to be safe to attach to a support request (`docs/logging.md`).
    fn no_store(operation: &str, _account_id: &str) -> Result<(), CredentialStoreError> {
        log::error!(
            "this build of Allodia Mail & Calendar for Linux has no secure store open, so it \
             cannot {operation} an account's saved sign-in",
        );
        Err(CredentialStoreError::Store(
            "no secure store is open".to_owned(),
        ))
    }
}

impl AccountCredentialStore for NoSecretStore {
    fn persist(
        &self,
        account_id: String,
        _config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        Self::no_store("write", &account_id)
    }

    fn delete(&self, account_id: String) -> Result<(), CredentialStoreError> {
        Self::no_store("erase", &account_id)
    }
}

/// The store the core writes through on a fixture launch.
///
/// A keyring that would not open leaves the refusal in place, so the failure is still reported
/// rather than dropped in silence.
pub(super) fn dev_credential_store(
    secrets: Option<&Arc<SecretStore>>,
) -> Box<dyn AccountCredentialStore> {
    match secrets {
        Some(store) => Box::new(SecretSink::new(Arc::clone(store))),
        None => Box::new(NoSecretStore),
    }
}

/// The canned account list a fixture launch connects, plus the one stored entry that is not a mail
/// account.
///
/// Which entry that is is the core's answer rather than a shape matched here: a client reading the
/// stored form itself would be a second reader of it, free to disagree the moment either moves.
/// **Only** that entry is carried over; appending the whole namespace would reconnect every
/// fixture account an earlier session left behind.
pub(super) fn with_stored_allodia_account(
    configs: Vec<String>,
    secrets: Option<&Arc<SecretStore>>,
) -> Vec<String> {
    let Some(store) = secrets else {
        return configs;
    };
    match store.configs() {
        Ok(stored) => carry_over_allodia_account(configs, stored),
        Err(error) => {
            log::warn!("this debug build could not read its own secure store: {error}");
            configs
        }
    }
}

/// The filter itself, away from the store so it can be tested without one.
fn carry_over_allodia_account(mut configs: Vec<String>, stored: Vec<String>) -> Vec<String> {
    configs.extend(
        stored
            .into_iter()
            .filter(|config| mailcal_bindings::is_allodia_account_config(config.clone())),
    );
    configs
}

#[cfg(test)]
mod tests {
    use super::carry_over_allodia_account;

    /// A fixture launch connects its canned accounts and **one** stored entry: the Allodia account.
    ///
    /// Both halves matter. Dropping it is what made a harness sign-in look like it never stuck,
    /// the grant was in the keyring and nothing put it back. Taking the whole namespace instead is
    /// the other failure: measured on macOS while getting this wrong, it connected five accounts,
    /// four of them fixtures left behind by earlier sessions.
    #[test]
    fn a_fixture_launch_carries_over_the_allodia_account_and_nothing_else() {
        let canned = "[jmap]\nurl = \"http://127.0.0.1:28080\"\n".to_owned();
        let allodia =
            "[allodia]\nemail = \"someone@allodia.test\"\nrefresh_token = \"grant\"\n".to_owned();
        let stale_fixture = "[imap]\naddr = \"imap.example.test:993\"\n".to_owned();

        let connected =
            carry_over_allodia_account(vec![canned.clone()], vec![stale_fixture, allodia.clone()]);

        assert_eq!(connected, vec![canned.clone(), allodia]);
    }

    /// An empty store leaves the canned list exactly as it was; the first launch after this
    /// change, and every launch by anyone who never signs in.
    #[test]
    fn a_fixture_launch_with_nothing_stored_connects_only_its_canned_accounts() {
        let canned = "[jmap]\nurl = \"http://127.0.0.1:28080\"\n".to_owned();

        assert_eq!(
            carry_over_allodia_account(vec![canned.clone()], Vec::new()),
            vec![canned]
        );
    }
}
