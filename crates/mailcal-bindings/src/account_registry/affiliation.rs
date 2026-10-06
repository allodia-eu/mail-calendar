//! Recording whether a Microsoft or Google account is a personal one, once its provider has said.

use engine_api::{AccountId, Affiliation};

use super::AccountRegistry;
use crate::ConnectedAccount;

impl AccountRegistry {
    /// Records `affiliation` on `id` and re-serializes its config for the host's store.
    ///
    /// `None` when there is nothing to write: the account is gone, is neither a Microsoft nor a
    /// Google one, or already holds that answer.
    pub(crate) fn record_affiliation(
        &self,
        id: &AccountId,
        affiliation: &Affiliation,
    ) -> Option<Result<String, String>> {
        let mut entries = self.entries.lock().ok()?;
        let entry = entries.get_mut(id.as_str())?;
        let stored = match entry {
            ConnectedAccount::Microsoft { config, .. } => &mut config.affiliation,
            ConnectedAccount::Google { config, .. } => &mut config.affiliation,
            ConnectedAccount::Imap { .. } | ConnectedAccount::Jmap { .. } => return None,
        };
        if stored.as_ref() == Some(affiliation) {
            return None;
        }
        *stored = Some(affiliation.clone());
        Some(entry.to_toml().map_err(|err| err.to_string()))
    }
}
