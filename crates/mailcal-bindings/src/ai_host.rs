//! What the bindings give the writing assistant (`mailcal_app::ai::AiHost`): the socket, the
//! runtime and the keystore entry its own endpoint's key is kept in.
//!
//! The key is a secret, so it is an entry in the host's secure store under a reserved id, as the
//! Allodia grant is, written through the one port every credential takes
//! ([`crate::credential_store`]). It is taken out of the stored configs at launch before any mail
//! parser sees it ([`take_stored`]), and a client asks [`is_reserved_config`] rather than
//! recognising the entry itself.

use std::{future::Future, pin::Pin, sync::Arc};

use mailcal_ai::HttpTransport;
use mailcal_app::ai::{AiCancel, AiHost, BuildKind, KeyNotStored};
use serde::{Deserialize, Serialize};

use crate::{ai_transport::AiTransport, credential_store::AccountCredentialStore};

/// The secure-store id the key is kept under.
const KEY_ID: &str = "ai-endpoint";

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
        .filter(|key| !key.trim().is_empty())
}

/// Takes the own endpoint's key out of the host's stored configs, leaving the mail accounts.
/// Called at launch, unconditionally, for the reason `allodia::take_stored` is.
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
/// or the own endpoint's key.
///
/// A client asks it where it counts mail accounts (a first run is one with none) and where a
/// debug launch that connects a canned account carries the stored entries that are not mail
/// accounts over. Asked rather than matched, because the stored shapes are the core's.
#[must_use]
#[uniffi::export]
pub fn is_reserved_config(config: String) -> bool {
    from_toml(&config).is_some() || crate::allodia::is_allodia_account_config(config)
}

/// The build this core is, for the offer (`docs/ai.md` rule 5).
pub(crate) const fn build_kind() -> BuildKind {
    if cfg!(any(debug_assertions, feature = "dev-harness")) {
        BuildKind::Development
    } else {
        BuildKind::Production
    }
}

/// The host the assistant runs on.
pub(crate) struct BindingsAiHost {
    handle: tokio::runtime::Handle,
    credential_store: Arc<dyn AccountCredentialStore>,
}

impl BindingsAiHost {
    pub(crate) fn new(
        handle: tokio::runtime::Handle,
        credential_store: Arc<dyn AccountCredentialStore>,
    ) -> Self {
        Self {
            handle,
            credential_store,
        }
    }
}

impl AiHost for BindingsAiHost {
    fn transport(&self, cancel: AiCancel) -> Box<dyn HttpTransport> {
        match AiTransport::new(self.handle.clone(), cancel) {
            Ok(transport) => Box::new(transport),
            Err(_) => {
                log::warn!("ai: no client could be built over the TLS policy");
                Box::new(NoTransport)
            }
        }
    }

    fn wait(&self, work: Pin<Box<dyn Future<Output = ()> + Send + '_>>) {
        crate::runtime::drive(&self.handle, move || work);
    }

    fn store_endpoint_key(&self, key: Option<&str>) -> Result<(), KeyNotStored> {
        let written = match key {
            Some(key) => toml::to_string(&StoredKey {
                ai_endpoint: KeyTable {
                    key: key.to_owned(),
                },
            })
            .map_err(|_| KeyNotStored)
            .and_then(|stored| {
                self.credential_store
                    .persist(KEY_ID.to_owned(), stored)
                    .map_err(|_| KeyNotStored)
            }),
            None => self
                .credential_store
                .delete(KEY_ID.to_owned())
                .map_err(|_| KeyNotStored),
        };
        if written.is_err() {
            log::warn!("ai: the keystore refused the endpoint's key");
        }
        written
    }
}

/// A transport for a build whose TLS policy yields no client: every request is no answer.
struct NoTransport;

impl HttpTransport for NoTransport {
    fn post_json(
        &self,
        _request: mailcal_ai::HttpRequest<'_>,
    ) -> Result<mailcal_ai::HttpResponse, mailcal_ai::TransportFailed> {
        Err(mailcal_ai::TransportFailed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use mailcal_app::ai::AiHost;

    use super::{BindingsAiHost, is_reserved_config, take_stored};
    use crate::credential_store::{AccountCredentialStore, CredentialStoreError};

    const KEY: &str = "[ai_endpoint]\nkey = \"sk-local\"\n";

    #[derive(Default)]
    struct Store(Mutex<Vec<(String, Option<String>)>>);

    impl AccountCredentialStore for Store {
        fn persist(&self, id: String, config: String) -> Result<(), CredentialStoreError> {
            self.0.lock().unwrap().push((id, Some(config)));
            Ok(())
        }

        fn delete(&self, id: String) -> Result<(), CredentialStoreError> {
            self.0.lock().unwrap().push((id, None));
            Ok(())
        }
    }

    #[test]
    fn the_key_is_taken_out_of_the_stored_configs_before_any_mail_parser_sees_it() {
        let mut configs = vec!["[imap]\nhost = \"x\"\n".to_owned(), KEY.to_owned()];
        assert_eq!(take_stored(&mut configs).as_deref(), Some("sk-local"));
        assert_eq!(configs, ["[imap]\nhost = \"x\"\n"]);
    }

    #[test]
    fn both_reserved_entries_are_recognised_and_a_mail_account_is_not() {
        assert!(is_reserved_config(KEY.to_owned()));
        assert!(is_reserved_config(
            "[allodia]\nemail = \"a@b.eu\"\nrefresh_token = \"r\"\n".to_owned()
        ));
        assert!(!is_reserved_config("[imap]\nhost = \"x\"\n".to_owned()));
        assert!(!is_reserved_config(
            "[ai_endpoint]\nkey = \" \"\n".to_owned()
        ));
    }

    /// What the host writes is what the next launch takes back out.
    #[test]
    fn a_key_written_reads_back_at_the_next_launch_and_a_removal_deletes_the_entry() {
        let store = Arc::new(Store::default());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let host = BindingsAiHost::new(
            runtime.handle().clone(),
            Arc::clone(&store) as Arc<dyn AccountCredentialStore>,
        );
        host.store_endpoint_key(Some("sk-new")).unwrap();
        host.store_endpoint_key(None).unwrap();
        let written = store.0.lock().unwrap().clone();
        assert_eq!(written.len(), 2);
        assert_eq!(written[0].0, "ai-endpoint");
        let mut configs = vec![written[0].1.clone().unwrap()];
        assert_eq!(take_stored(&mut configs).as_deref(), Some("sk-new"));
        assert_eq!(written[1], ("ai-endpoint".to_owned(), None));
    }
}
