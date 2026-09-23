//! The person's own AI endpoint, and which backend the app's AI requests go through.
//!
//! The endpoint's address, model and declaration are preferences the core keeps. Its key is a
//! secret, so it is an entry in the host's secure store under a reserved id, like the Allodia
//! grant, and is taken out of the stored configs at boot before any mail parser sees it
//! ([`take_stored`]). A client asks [`is_reserved_config`] rather than recognising either entry
//! itself.
//!
//! **Which backend.** An own endpoint, when one is set up, wins: the person configured it
//! explicitly. Whatever is chosen is handed to the core as a `GatedBackend`, so every request
//! passes the jurisdiction gate on the way out (`docs/ai.md`).

use mailcal_ai::{EndpointError, GatedBackend, OwnEndpoint};
use serde::{Deserialize, Serialize};

use crate::{MailcalApp, ai_transport::AiTransport, records_writing_style::JurisdictionClass};

/// The secure-store id the key is kept under.
pub(crate) const KEY_ID: &str = "ai-endpoint";

/// The stored document: one `[ai_endpoint]` table, which no mail config has.
#[derive(Serialize, Deserialize)]
struct StoredKey {
    ai_endpoint: KeyTable,
}

#[derive(Serialize, Deserialize)]
struct KeyTable {
    key: String,
}

fn from_toml(config: &str) -> Option<String> {
    toml::from_str::<StoredKey>(config)
        .ok()
        .map(|stored| stored.ai_endpoint.key)
        .filter(|key| !key.is_empty())
}

/// Takes the own endpoint's key out of the host's stored configs, leaving the mail accounts.
/// Called at boot, unconditionally, for the reason `allodia::take_stored` is.
pub(crate) fn take_stored(configs: &mut Vec<String>) -> Option<String> {
    let mut found = None;
    configs.retain(|config| match from_toml(config) {
        Some(key) => {
            found = found.take().or(Some(key));
            false
        }
        None => true,
    });
    found
}

/// Whether a stored entry is one of the core's own rather than a mail account: the Allodia grant
/// or an own AI endpoint's key.
///
/// Two paths need it. **Deciding a first run**: a client shows the account-setup screen when the
/// store holds no mail account, and the store holds these entries too, so the number of entries is
/// not the number of mail accounts. Read as though it were, signing in on the first-run screen and
/// quitting before adding a mailbox left the next launch convinced setup was finished, with no way
/// back to the screen that adds an account. **A debug launch that connects a canned dev account**
/// replaces the stored accounts and has to carry these entries over, or a sign-in made in that
/// mode looks like it never stuck.
///
/// Asked rather than pattern-matched, because the stored shapes belong to the core: a client that
/// looked for a section name itself would be a second reader, free to disagree.
#[must_use]
#[uniffi::export]
pub fn is_reserved_config(config: String) -> bool {
    from_toml(&config).is_some() || crate::allodia::StoredAccount::from_toml(&config).is_some()
}

/// The own endpoint's settings, as the Advanced screen shows them. The key itself never crosses
/// back: `has_key` says whether one is stored.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OwnAiEndpoint {
    /// The API root.
    pub base_url: String,
    /// The model asked for.
    pub model: String,
    /// Where the person says it runs.
    pub declared: Option<JurisdictionClass>,
    /// Whether a key is stored.
    pub has_key: bool,
}

/// Why the own endpoint's settings were not saved.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error, thiserror::Error)]
pub enum OwnEndpointError {
    /// The address is not a usable URL.
    #[error("invalid url")]
    InvalidUrl,
    /// The address is plain HTTP to somewhere other than this device.
    #[error("not https")]
    NotHttps,
    /// No model name was given.
    #[error("no model")]
    NoModel,
    /// The secure store refused the key.
    #[error("keystore: {0}")]
    Keystore(String),
}

impl From<EndpointError> for OwnEndpointError {
    fn from(error: EndpointError) -> Self {
        match error {
            EndpointError::InvalidUrl => Self::InvalidUrl,
            EndpointError::NotHttps => Self::NotHttps,
            EndpointError::NoModel => Self::NoModel,
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Whether AI requests have somewhere to go.
    #[must_use]
    pub fn ai_available(&self) -> bool {
        self.app.ai_available()
    }

    /// The own endpoint's settings, or `None` when none is set up.
    #[must_use]
    pub fn own_ai_endpoint(&self) -> Option<OwnAiEndpoint> {
        let stored = self.app.own_ai_endpoint()?;
        Some(OwnAiEndpoint {
            base_url: stored.base_url,
            model: stored.model,
            declared: stored.declared.map(Into::into),
            has_key: self.ai_key.lock().expect("ai key lock").is_some(),
        })
    }

    /// Saves the own endpoint and switches AI requests to it.
    ///
    /// `key` replaces the stored key when given, and an empty one removes it; `None` keeps the key
    /// already stored, so the Advanced screen can save an edit without asking for it again.
    ///
    /// # Errors
    ///
    /// Returns [`OwnEndpointError`] naming the first setting that is not usable, in which case
    /// nothing was saved, or the secure store's refusal of the key.
    pub fn set_own_ai_endpoint(
        &self,
        base_url: String,
        model: String,
        declared: Option<JurisdictionClass>,
        key: Option<String>,
    ) -> Result<(), OwnEndpointError> {
        let declared = declared.map(Into::into);
        let current = self.ai_key.lock().expect("ai key lock").clone();
        let next = match &key {
            Some(key) => Some(key.trim().to_owned()).filter(|key| !key.is_empty()),
            None => current,
        };
        let endpoint = OwnEndpoint::new(&base_url, next.clone(), &model, declared)?;
        if key.is_some() {
            self.store_ai_key(next.as_deref())?;
        }
        self.app
            .set_own_ai_endpoint(Some(mailcal_account::AiEndpoint {
                base_url: endpoint.base_url().to_owned(),
                model: endpoint.model().to_owned(),
                declared,
            }));
        log::info!("ai: an own endpoint was saved");
        self.refresh_ai_backend();
        Ok(())
    }

    /// Removes the own endpoint and its key.
    ///
    /// # Errors
    ///
    /// Returns [`OwnEndpointError::Keystore`] when the secure store refused to delete the key; the
    /// settings are removed either way.
    pub fn clear_own_ai_endpoint(&self) -> Result<(), OwnEndpointError> {
        self.app.set_own_ai_endpoint(None);
        let deleted = self.store_ai_key(None);
        log::info!("ai: the own endpoint was removed");
        self.refresh_ai_backend();
        deleted
    }
}

impl MailcalApp {
    /// Writes `key` to the secure store, or deletes it with `None`, and keeps it in memory.
    fn store_ai_key(&self, key: Option<&str>) -> Result<(), OwnEndpointError> {
        let result = match key {
            Some(key) => toml::to_string(&StoredKey {
                ai_endpoint: KeyTable {
                    key: key.to_owned(),
                },
            })
            .map_err(|error| OwnEndpointError::Keystore(error.to_string()))
            .and_then(|stored| {
                self.credential_store
                    .persist(KEY_ID.to_owned(), stored)
                    .map_err(|error| OwnEndpointError::Keystore(error.to_string()))
            }),
            None => self
                .credential_store
                .delete(KEY_ID.to_owned())
                .map_err(|error| OwnEndpointError::Keystore(error.to_string())),
        };
        if result.is_ok() || key.is_none() {
            *self.ai_key.lock().expect("ai key lock") = key.map(str::to_owned);
        }
        result
    }

    /// Builds the backend AI requests go through from what is set up now, and hands it to the
    /// core; `None` when nothing is.
    pub(crate) fn refresh_ai_backend(&self) {
        let key = self.ai_key.lock().expect("ai key lock").clone();
        let backend = self
            .app
            .own_ai_endpoint()
            .and_then(|stored| {
                OwnEndpoint::new(&stored.base_url, key, &stored.model, stored.declared).ok()
            })
            .and_then(|endpoint| {
                let transport = AiTransport::new(self.runtime.handle().clone()).ok()?;
                Some(GatedBackend::own_endpoint(
                    endpoint,
                    Box::new(transport),
                    self.app.jurisdiction_mode_source(),
                ))
            });
        self.app.set_ai_backend(backend);
    }
}

#[cfg(test)]
mod tests {
    use super::{is_reserved_config, take_stored};

    const KEY: &str = "[ai_endpoint]\nkey = \"sk-local\"\n";

    #[test]
    fn the_key_is_taken_out_of_the_stored_configs_before_any_mail_parser_sees_it() {
        let mut configs = vec!["[imap]\nhost = \"x\"\n".to_owned(), KEY.to_owned()];
        assert_eq!(take_stored(&mut configs).as_deref(), Some("sk-local"));
        assert_eq!(configs.len(), 1);
        assert!(configs[0].starts_with("[imap]"));
    }

    #[test]
    fn both_reserved_entries_are_recognised_and_a_mail_account_is_not() {
        assert!(is_reserved_config(KEY.to_owned()));
        assert!(is_reserved_config(
            "[allodia]\nemail = \"a@b.eu\"\nrefresh_token = \"r\"\n".to_owned()
        ));
        assert!(!is_reserved_config("[imap]\nhost = \"x\"\n".to_owned()));
        assert!(!is_reserved_config(
            "[ai_endpoint]\nkey = \"\"\n".to_owned()
        ));
    }
}
