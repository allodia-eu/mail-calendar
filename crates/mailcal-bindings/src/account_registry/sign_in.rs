//! The stored config an account's sign-in is switched, or repeated, from.

use mailcal_account::{AccountConfig, JmapAccountConfig};

use super::AccountRegistry;
use crate::ConnectedAccount;

/// The config of an account whose sign-in Settings can switch: a standards account or a JMAP
/// one. Microsoft and Google sign in only through their own consent.
pub(crate) enum SignInConfig {
    Imap(Box<AccountConfig>),
    Jmap(Box<JmapAccountConfig>),
}

impl AccountRegistry {
    /// `id`'s config, cloned out from under the lock, when its sign-in is one Settings can
    /// switch; `None` for an unknown account and for Microsoft and Google.
    pub(crate) fn sign_in_config(&self, id: &str) -> Option<SignInConfig> {
        match self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)?
        {
            ConnectedAccount::Imap { config, .. } => {
                Some(SignInConfig::Imap(Box::new(config.clone())))
            }
            ConnectedAccount::Jmap { config, .. } => {
                Some(SignInConfig::Jmap(Box::new(config.clone())))
            }
            ConnectedAccount::Microsoft { .. } | ConnectedAccount::Google { .. } => None,
        }
    }

    /// Records the autoconfig issuer the sign-in options found for `id`, or that there is none.
    pub(crate) fn remember_sign_in_issuer(&self, id: &str, issuer: Option<String>) {
        self.sign_in_issuers
            .lock()
            .expect("sign-in issuers mutex poisoned")
            .insert(id.to_owned(), issuer);
    }

    /// The autoconfig issuer recorded for `id`: `None` when nothing has looked yet, and
    /// `Some(None)` when it looked and found none.
    pub(crate) fn sign_in_issuer(&self, id: &str) -> Option<Option<String>> {
        self.sign_in_issuers
            .lock()
            .expect("sign-in issuers mutex poisoned")
            .get(id)
            .cloned()
    }

    /// Forgets what was asked about `id`'s servers, for an account registered again or removed:
    /// its calendar users and its sign-in issuer.
    pub(super) fn forget_answers(&self, id: &str) {
        self.forget_calendar_users(id);
        self.sign_in_issuers
            .lock()
            .expect("sign-in issuers mutex poisoned")
            .remove(id);
    }
}
