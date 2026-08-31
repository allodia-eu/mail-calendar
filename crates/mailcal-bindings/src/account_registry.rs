//! The account registry: the binding layer's record of how to re-open every connected account,
//! and the one gate every dial has to pass through.
//!
//! # Why this is a type and not a `HashMap`
//!
//! An `Arc<Mutex<HashMap<String, ConnectedAccount>>>` behind an alias puts `lock().insert(…)`
//! within reach of every module in the crate, and each one then holds its own opinion about *when*
//! an account becomes findable. Two ways of getting that wrong cost a grant, and nothing in an open
//! map can tell a reader which callers have avoided them:
//!
//! - Connecting before registering leaves the first rotation of a newly added account with no entry
//!   to land in, so it is dropped, and a provider that treats a replayed refresh token as theft
//!   revokes the whole grant.
//! - Re-inserting the config a connect parsed puts a token that a rotation has already replaced
//!   back over the good one.
//!
//! So:
//!
//! - **[`AccountRegistry::pre_register`] is the only way in.** There is no `insert`, no `get_mut`,
//!   and no way to reach the map.
//! - **It hands back a [`Registered`] token, and [`AccountDial`] can only be obtained from the
//!   registry.** So "the account is registered before anything dials it" is not a rule a reader has
//!   to know: an unregistered account has no dial to run.
//! - **Nothing writes a whole entry after a connect.** Committing consumes the rollback token and
//!   leaves the pre-registered entry in place; token rotation can update only its grant.
//!
//! # The store gets what the registry holds, never what the caller passed in
//!
//! [`MailcalApp::persist_registered_grant`] serializes out of the registry rather than re-using the
//! TOML a caller handed in. A connect refreshes; a refresh can rotate; a rotation advances the
//! registry's copy through the token sink. The caller's string was parsed before any of that, so
//! writing it would put the *superseded* refresh token back, which on a replay-detecting server is
//! worse than never having written anything.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use engine_api::AccountId;
use mailcal_account::{AccountConfig, Secret};

use crate::{AccountProvider, ConnectedAccount};

mod credentials;
mod dial;

pub(crate) use dial::{AccountDial, dial_all};

/// Every connected account's re-connection state, keyed by account id, and the gate that makes
/// "registered before dialed" a property of the code rather than a rule in a document.
///
/// See the module header for what the open `HashMap` this replaced cost.
#[derive(Debug, Default)]
pub(crate) struct AccountRegistry {
    entries: Mutex<HashMap<String, ConnectedAccount>>,
}

/// What one rotation did to the registry, for the token sink to report and persist.
pub(crate) enum Rotation {
    /// The entry advanced: the provider family (for the log) and the config to store.
    Advanced {
        /// `graph` / `google` / `jmap`: the protocol, never an endpoint or an address.
        family: &'static str,
        /// The whole config, re-serialized with the new refresh token in place.
        config_toml: String,
    },
    /// The account is registered but has nothing to rotate (an IMAP account, or a JMAP one signed
    /// in with a stored password), or its config could not be encoded.
    Nothing {
        /// Set when the failure is worth reporting; `None` for an account that simply has no
        /// grant.
        encode_error: Option<String>,
        /// The provider family, when known, for the log line.
        family: &'static str,
    },
    /// **No entry.** The rotation cannot be saved and the grant is lost at the next launch. This
    /// should be unreachable (see the module header) which is exactly why the sink shouts.
    Unregistered,
}

impl AccountRegistry {
    /// An empty registry, shared.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Writes `entry` into the registry **before** the connect that may rotate its credential, and
    /// returns the token that proves it is there.
    ///
    /// The only way to add an account. It takes the entry by value and returns nothing borrowed, so
    /// a caller cannot keep a second copy to write back later.
    pub(crate) fn pre_register(&self, id: String, entry: ConnectedAccount) -> Registered {
        let replaced = self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .insert(id.clone(), entry);
        Registered { id, replaced }
    }

    /// Forgets `id`: the account has been removed.
    pub(crate) fn remove(&self, id: &str) {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .remove(id);
    }

    /// How to dial `id`, or `None` if it is not registered.
    ///
    /// **The only constructor of an [`AccountDial`].** That is the enforcement: every path that
    /// opens a socket for an account (boot, reconnect, add, re-auth) must come through here, so
    /// an account that has not been registered cannot be dialed, and the ordering the token sink
    /// depends on cannot be got wrong by a new caller who has not read the header above.
    ///
    /// The dial is a *snapshot* (configs cloned, token sources `Arc`-shared) so the caller holds no
    /// lock across the network round trip.
    pub(crate) fn dial(&self, id: &str) -> Option<AccountDial> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .map(AccountDial::from_entry)
    }

    /// Whether `id` is still registered, asked after a slow dial, in case the user removed the
    /// account while it ran.
    pub(crate) fn contains(&self, id: &str) -> bool {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .contains_key(id)
    }

    /// The provider family of `id` (`imap` / `graph` / `google` / `jmap`), safe for a diagnostic
    /// log because it names only the protocol.
    pub(crate) fn account_type(&self, id: &str) -> &'static str {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .map_or("unknown", ConnectedAccount::account_type)
    }

    /// Which kind of sign-in `id` was connected with, for the host's reconnect prompt.
    pub(crate) fn provider(&self, id: &str) -> Option<AccountProvider> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .map(ConnectedAccount::provider)
    }

    /// Every account's `(id, protocol)`, for the analytics account mix. The ids stay on this side
    /// of the boundary: an id embeds an address.
    pub(crate) fn protocols(&self) -> std::collections::BTreeMap<String, mailcal_app::Protocol> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .map(|(id, account)| (id.clone(), account.protocol()))
            .collect()
    }

    /// `id`'s IMAP config, or `None` for an account with no IMAP half: the filter a standing
    /// `IDLE` watch needs, since Graph and Google poll instead.
    pub(crate) fn imap_config(&self, id: &str) -> Option<AccountConfig> {
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
            .and_then(|entry| entry.imap().cloned())
    }

    /// `id`'s registered JMAP config, cloned: the precondition the JMAP re-authentication path
    /// checks before it spends an authorization code, and the source of the persisted grant it
    /// rebuilds its authorization URL from.
    ///
    /// `Err` names which of the two reasons applies, because a host shows them differently: an
    /// account of another family has no JMAP sign-in to re-run at all, while an unknown id means
    /// the user removed it while the browser was open.
    pub(crate) fn jmap_config(
        &self,
        id: &str,
    ) -> Result<mailcal_account::JmapAccountConfig, JmapLookup> {
        match self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)
        {
            Some(ConnectedAccount::Jmap { config, .. }) => Ok(config.clone()),
            Some(_) => Err(JmapLookup::NotJmap),
            None => Err(JmapLookup::Unknown),
        }
    }

    /// Re-serializes a password or pasted-secret account with `secret`, without changing the
    /// registered entry. The caller dials this candidate first and only commits it after the
    /// server accepts it, so a typo cannot replace the credential the user is trying to repair.
    pub(crate) fn replacement_secret_toml(&self, id: &str, secret: &str) -> Result<String, String> {
        if secret.is_empty() {
            return Err("the replacement credential is empty".to_owned());
        }
        match self
            .entries
            .lock()
            .map_err(|_| "the account registry is unavailable".to_owned())?
            .get(id)
        {
            Some(ConnectedAccount::Imap(config)) => config
                .with_password(secret)
                .to_toml()
                .map_err(|error| error.to_string()),
            Some(ConnectedAccount::Jmap { config, .. }) if !config.is_oauth() => {
                let mut updated = config.clone();
                updated.password = Some(Secret::new(secret.to_owned()));
                // New setup credentials use the username-bearing field, which can negotiate
                // Basic or Bearer. Replacing a legacy bearer-only token migrates it there.
                updated.token = None;
                updated.to_toml().map_err(|error| error.to_string())
            }
            Some(ConnectedAccount::Jmap { .. }) => {
                Err("this JMAP account must be repaired through browser sign-in".to_owned())
            }
            Some(ConnectedAccount::Microsoft { .. } | ConnectedAccount::Google { .. }) => {
                Err("this account must be repaired through browser sign-in".to_owned())
            }
            None => Err("no such account".to_owned()),
        }
    }

    /// Every account's config, re-serialized, keyed by account id.
    ///
    /// Out of the registry rather than out of the host's stored strings, for the same reason
    /// [`MailcalApp::persist_registered_grant`] is: a connect refreshes and a refresh can rotate,
    /// so the caller's original string is the *superseded* one. An account whose config will not
    /// serialize is left out and named, because a silent gap here is an account that stops syncing
    /// with nothing to read about why.
    #[cfg(feature = "allodia-license")]
    pub(crate) fn stored_configs(&self) -> std::collections::BTreeMap<String, String> {
        let entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let mut configs = std::collections::BTreeMap::new();
        for (id, entry) in entries.iter() {
            let serialized = match entry {
                ConnectedAccount::Imap(config) => config.to_toml(),
                ConnectedAccount::Microsoft { config, .. } => config.to_toml(),
                ConnectedAccount::Google { config, .. } => config.to_toml(),
                ConnectedAccount::Jmap { config, .. } => config.to_toml(),
            };
            match serialized {
                Ok(toml) => {
                    configs.insert(id.clone(), toml);
                }
                Err(error) => log::warn!(
                    "credentials: [{}] this account's config could not be re-serialized; {error}",
                    mailcal_account::account_log_handle(id),
                ),
            }
        }
        configs
    }

    /// `id`'s registered OAuth config, re-serialized: the bytes
    /// [`MailcalApp::persist_registered_grant`] writes to the host's store.
    ///
    /// # Errors
    ///
    /// Returns a message naming why when `id` is not a registered OAuth account, or its config
    /// cannot be serialized. Named rather than silently skipped: a no-op here would look exactly
    /// like a successful write.
    pub(crate) fn oauth_config_toml(&self, id: &str) -> Result<String, String> {
        let entries = self
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        let serialized = match entries.get(id) {
            Some(ConnectedAccount::Microsoft { config, .. }) => config.to_toml(),
            Some(ConnectedAccount::Google { config, .. }) => config.to_toml(),
            Some(ConnectedAccount::Jmap { config, .. }) => config.to_toml(),
            Some(ConnectedAccount::Imap(_)) | None => {
                return Err(format!(
                    "no registered OAuth account to persist for id {id}"
                ));
            }
        };
        serialized.map_err(|err| err.to_string())
    }

    /// Advances `id`'s stored refresh token to `new_refresh_token` and re-serializes its config,
    /// under one lock and never across the foreign store write that follows.
    ///
    /// The registry is the only holder of the durable half of a credential, so this is the only
    /// place a rotation is recorded, and it returns what it did rather than logging, so the sink
    /// owns the wording that reaches a user's device.
    pub(crate) fn rotate_refresh_token(&self, id: &AccountId, new_refresh_token: &str) -> Rotation {
        let Ok(mut entries) = self.entries.lock() else {
            return Rotation::Unregistered;
        };
        let (family, encoded) = match entries.get_mut(id.as_str()) {
            Some(ConnectedAccount::Microsoft { config, .. }) => {
                config.refresh_token = Secret::new(new_refresh_token.to_owned());
                ("graph", config.to_toml())
            }
            Some(ConnectedAccount::Google { config, .. }) => {
                config.refresh_token = Secret::new(new_refresh_token.to_owned());
                ("google", config.to_toml())
            }
            // An OAuth JMAP account rotates through the same sink; a stored-secret one has no
            // grant to update.
            Some(ConnectedAccount::Jmap { config, .. }) => match config.oauth.as_mut() {
                Some(grant) => {
                    grant.refresh_token = Secret::new(new_refresh_token.to_owned());
                    ("jmap", config.to_toml())
                }
                None => {
                    return Rotation::Nothing {
                        encode_error: None,
                        family: "jmap",
                    };
                }
            },
            // A password account has nothing that can rotate; reaching here at all would be a bug
            // in the caller, not a lost credential.
            Some(ConnectedAccount::Imap(_)) => {
                return Rotation::Nothing {
                    encode_error: None,
                    family: "imap",
                };
            }
            None => return Rotation::Unregistered,
        };
        match encoded {
            Ok(config_toml) => Rotation::Advanced {
                family,
                config_toml,
            },
            Err(err) => Rotation::Nothing {
                encode_error: Some(err.to_string()),
                family,
            },
        }
    }
}

/// Why [`AccountRegistry::jmap_config`] found no JMAP account.
#[derive(Debug)]
pub(crate) enum JmapLookup {
    /// The account exists but is not a JMAP one, so it has no JMAP sign-in to re-run.
    NotJmap,
    /// No such account; removed while the browser was open.
    Unknown,
}

/// A registry entry written **before** its connect, holding whatever it displaced so a failed
/// connect can put the registry back exactly as it was.
///
/// The displaced entry matters more than it looks. `complete_microsoft_login` is *also* the
/// re-authentication path (the host re-runs sign-in for an account it already has) so an entry is
/// routinely there already, and a rollback that merely *removed* would delete a live account's
/// re-connection state because its re-auth failed. Re-adding an existing account through
/// `add_account` is the same shape.
pub(crate) struct Registered {
    id: String,
    replaced: Option<ConnectedAccount>,
}

impl Registered {
    /// Undoes the pre-registration after a failed connect: restores the entry that was displaced,
    /// or removes the one this added.
    pub(crate) fn rollback(self, registry: &AccountRegistry) {
        let mut entries = registry
            .entries
            .lock()
            .expect("account registry mutex poisoned");
        match self.replaced {
            Some(previous) => entries.insert(self.id, previous),
            None => entries.remove(&self.id),
        };
    }

    /// Commits the registration by consuming the rollback token. The entry already contains the
    /// live token source prepared before the dial; committing must not replace any part of it.
    pub(crate) fn commit(self) {}
}

#[cfg(test)]
#[path = "account_registry/tests.rs"]
mod tests;
