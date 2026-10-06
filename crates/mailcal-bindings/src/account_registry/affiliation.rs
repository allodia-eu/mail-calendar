//! Recording whether a Microsoft account is a personal one, once Graph has said.

use engine_api::{AccountId, Affiliation};

use super::AccountRegistry;
use crate::ConnectedAccount;

impl AccountRegistry {
    /// Records `affiliation` on `id` and re-serializes its config for the host's store.
    ///
    /// `None` when there is nothing to write: the account is gone, is not a Microsoft one, or
    /// already holds that answer.
    pub(crate) fn record_affiliation(
        &self,
        id: &AccountId,
        affiliation: &Affiliation,
    ) -> Option<Result<String, String>> {
        let mut entries = self.entries.lock().ok()?;
        let ConnectedAccount::Microsoft { config, .. } = entries.get_mut(id.as_str())? else {
            return None;
        };
        if config.affiliation.as_ref() == Some(affiliation) {
            return None;
        }
        config.affiliation = Some(affiliation.clone());
        Some(config.to_toml().map_err(|err| err.to_string()))
    }
}
