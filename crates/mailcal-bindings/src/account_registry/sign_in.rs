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
}
