//! What the binding layer remembers about a connected account, so it can reconnect one.
//!
//! Split from `lib.rs`, which is the FFI object and its lifecycle: this is a value type three
//! other modules read (`account_registry`, `token_sink`, `boot`). It is also the only layer that
//! knows which protocol a `dyn Provider` actually speaks, which is why the "what kind of account
//! is this" answers live on it.

use std::sync::Arc;

use mailcal_account::{
    AccountConfig, GoogleConfig, GraphTokenSource, ImapConnections, JmapAccountConfig,
    MicrosoftConfig,
};

/// An IMAP account's config, connections and (for an OAuth account) token source, borrowed.
pub(crate) type ImapParts<'a> = (
    &'a AccountConfig,
    &'a Arc<ImapConnections>,
    Option<&'a Arc<GraphTokenSource>>,
);

/// One connected account's re-connection state, kept so the on-demand [`HostConnector`]
/// and a sync-depth change can re-open a provider for any of its folders after the fact.
/// An IMAP account carries its config; a Microsoft account carries its config plus the
/// shared, self-refreshing [`GraphTokenSource`] every one of its folder providers uses.
/// Both hold credentials in memory only (never logged; their `Debug` redacts secrets).
///
/// The IMAP variant is the wider one because it carries a whole standards config. Boxing it would
/// buy nothing: the registry holds one entry per account.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum ConnectedAccount {
    /// An IMAP/SMTP/CalDAV account and its connections.
    Imap {
        /// The persisted config (its refresh token is updated in place on rotation).
        config: AccountConfig,
        /// The account's IMAP connections, shared by every folder provider and push watch, so
        /// the account's socket count is the engine's budget however many folders are bound.
        connections: Arc<ImapConnections>,
        /// The shared token source every connection, watch and calendar of an **OAuth** account
        /// mints access tokens from; `None` for a password account, which has nothing to
        /// refresh.
        tokens: Option<Arc<GraphTokenSource>>,
    },
    /// A Microsoft 365 (Graph/OAuth) account and its shared token source.
    Microsoft {
        /// The persisted config (its refresh token is updated in place on rotation).
        config: MicrosoftConfig,
        /// The shared token source every folder provider (and on-demand open) refreshes
        /// through.
        tokens: Arc<GraphTokenSource>,
    },
    /// A Google (Gmail + Google Calendar / OAuth) account and its shared token source. Like
    /// Microsoft it refreshes through the (provider-neutral) [`GraphTokenSource`], but its mail
    /// provider is account-global (one provider, no per-folder fan-out: the JMAP shape).
    Google {
        /// The persisted config (its refresh token is updated in place on the rare rotation).
        config: GoogleConfig,
        /// The shared token source the Gmail + calendar providers (and on-demand open) refresh
        /// through.
        tokens: Arc<GraphTokenSource>,
    },
    /// A JMAP account. Carries its config so an on-demand folder open can reconnect a
    /// provider (there are no per-folder providers; one covers the account), plus, for an
    /// **OAuth** JMAP account, the shared self-refreshing token source its mail and calendar
    /// providers mint access tokens from. `tokens` is `None` for a stored-secret account,
    /// which has nothing to refresh.
    Jmap {
        /// The persisted config (its refresh token is updated in place on rotation).
        config: JmapAccountConfig,
        /// The shared token source, for an OAuth account only.
        tokens: Option<Arc<GraphTokenSource>>,
    },
}

impl ConnectedAccount {
    /// The account provider family, safe for diagnostic logs because it names only the
    /// protocol/provider kind, not an endpoint or user identity.
    pub(crate) const fn account_type(&self) -> &'static str {
        match self {
            Self::Imap { .. } => "imap",
            Self::Microsoft { .. } => "graph",
            Self::Google { .. } => "google",
            Self::Jmap { .. } => "jmap",
        }
    }

    /// The same provider family, as the core's analytics protocol. Safe for the same reason
    /// [`Self::account_type`] is: it names the protocol and nothing else. This binding layer is
    /// the only layer that knows which protocol a `dyn Provider` actually speaks, so it is the
    /// only layer that can answer this; hence `App::set_accounts`.
    pub(crate) const fn protocol(&self) -> mailcal_app::Protocol {
        match self {
            Self::Imap { .. } => mailcal_app::Protocol::Imap,
            Self::Microsoft { .. } => mailcal_app::Protocol::Graph,
            Self::Google { .. } => mailcal_app::Protocol::Google,
            Self::Jmap { .. } => mailcal_app::Protocol::Jmap,
        }
    }

    /// The same provider family as the host-facing [`AccountProvider`], which is what a host
    /// needs to know to re-run a sign-in the server has stopped accepting. Like
    /// [`Self::account_type`] it names only the family, never an endpoint or identity.
    ///
    /// JMAP splits in two, because JMAP is the one kind whose remedy is not decided by the
    /// protocol: an account connected by **signing in** can re-run that sign-in in place, while
    /// one holding a pasted password/API token has no browser flow at all and must be re-entered
    /// in Settings. Only the account's own config knows which it is.
    pub(crate) fn provider(&self) -> crate::AccountProvider {
        match self {
            Self::Imap { .. } => crate::AccountProvider::Password,
            Self::Microsoft { .. } => crate::AccountProvider::Microsoft,
            Self::Google { .. } => crate::AccountProvider::Google,
            Self::Jmap { config, .. } if config.is_oauth() => crate::AccountProvider::JmapOauth,
            Self::Jmap { .. } => crate::AccountProvider::Jmap,
        }
    }

    /// The keys every kind shares: pinned id, capabilities and links.
    pub(crate) const fn shape(&self) -> &mailcal_account::AccountShape {
        match self {
            Self::Imap { config, .. } => &config.shape,
            Self::Microsoft { config, .. } => &config.shape,
            Self::Google { config, .. } => &config.shape,
            Self::Jmap { config, .. } => &config.shape,
        }
    }

    /// The address the account is known by: the signed-in address, or a standards account's
    /// login.
    pub(crate) fn identity(&self) -> engine_api::EmailAddress {
        match self {
            Self::Imap { config, .. } => {
                engine_api::EmailAddress::new(config.username().to_owned())
            }
            Self::Microsoft { config, .. } => config.identity(),
            Self::Google { config, .. } => config.identity(),
            Self::Jmap { config, .. } => config.identity(),
        }
    }

    /// The account's id, pinned or derived; `None` only for a config no id can be derived from.
    pub(crate) fn account_id(&self) -> Option<engine_api::AccountId> {
        match self {
            Self::Imap { config, .. } => config.account_id().ok(),
            Self::Microsoft { config, .. } => config.account_id().ok(),
            Self::Google { config, .. } => config.account_id().ok(),
            Self::Jmap { config, .. } => config.account_id().ok(),
        }
    }

    /// The id the account's own settings derive, whatever id is pinned; `None` when none derives.
    pub(crate) fn derived_account_id(&self) -> Option<engine_api::AccountId> {
        match self {
            Self::Imap { config, .. } => config.derived_account_id().ok(),
            Self::Microsoft { config, .. } => config.derived_account_id().ok(),
            Self::Google { config, .. } => config.derived_account_id().ok(),
            Self::Jmap { config, .. } => config.derived_account_id().ok(),
        }
    }

    /// The same keys, to change in place.
    pub(crate) const fn shape_mut(&mut self) -> &mut mailcal_account::AccountShape {
        match self {
            Self::Imap { config, .. } => &mut config.shape,
            Self::Microsoft { config, .. } => &mut config.shape,
            Self::Google { config, .. } => &mut config.shape,
            Self::Jmap { config, .. } => &mut config.shape,
        }
    }

    /// The config as the host's store keeps it.
    pub(crate) fn to_toml(&self) -> Result<String, mailcal_account::ConfigError> {
        match self {
            Self::Imap { config, .. } => config.to_toml(),
            Self::Microsoft { config, .. } => config.to_toml(),
            Self::Google { config, .. } => config.to_toml(),
            Self::Jmap { config, .. } => config.to_toml(),
        }
    }

    /// What Settings → Accounts lists about the account `id`.
    pub(crate) fn facts(&self, id: &str) -> crate::accounts_view::AccountFacts {
        let chosen = self.capabilities();
        let (kind, files_invitations) = match self {
            Self::Imap { config, .. } if chosen.contains(mailcal_account::Capability::Mail) => {
                (crate::AccountKind::Imap, config.caldav.is_some())
            }
            Self::Imap { config, .. } => (crate::AccountKind::Dav, config.caldav.is_some()),
            Self::Microsoft { .. } => (crate::AccountKind::Microsoft, false),
            Self::Google { .. } => (crate::AccountKind::Google, false),
            Self::Jmap { .. } => (crate::AccountKind::Jmap, false),
        };
        let withheld = match self {
            Self::Microsoft { config, .. } => config.withheld_capabilities(),
            Self::Google { config, .. } => config.withheld_capabilities(),
            Self::Imap { .. } | Self::Jmap { .. } => mailcal_account::Capabilities::default(),
        };
        crate::accounts_view::AccountFacts {
            id: id.to_owned(),
            address: self.identity().email,
            kind,
            offered: self.offered(),
            chosen,
            withheld,
            files_invitations,
            links: self.shape().links.clone(),
            calendar_addresses: Vec::new(),
            endpoints: match self {
                Self::Imap { config, .. } if !config.is_oauth() => Some(config.endpoints()),
                _ => None,
            },
        }
    }

    /// What the account is used for: its stored choice, or what its kind has always meant.
    pub(crate) fn capabilities(&self) -> mailcal_account::Capabilities {
        match self {
            Self::Imap { config, .. } => config.capabilities(),
            Self::Microsoft { config, .. } => config.capabilities(),
            Self::Google { config, .. } => config.capabilities(),
            Self::Jmap { config, .. } => config.capabilities(),
        }
    }

    /// What the account can be used for: colleagues on a Google account and on a Microsoft one
    /// that is not personal, beside the mail, calendar and contacts every kind offers.
    pub(crate) fn offered(&self) -> mailcal_account::Capabilities {
        use mailcal_account::Capability;
        match self {
            Self::Microsoft { config, .. } => config.offered(),
            Self::Google { .. } => Capability::ALL.into_iter().collect(),
            Self::Imap { .. } | Self::Jmap { .. } => {
                [Capability::Mail, Capability::Calendar, Capability::Contacts]
                    .into_iter()
                    .collect()
            }
        }
    }

    /// What the account opens: what it is used for, less what a Microsoft or Google grant
    /// withholds. Every other kind opens all of what it is used for.
    pub(crate) fn opened_capabilities(&self) -> mailcal_account::Capabilities {
        match self {
            Self::Microsoft { config, .. } => {
                crate::consent::opened(&config.capabilities(), &config.withheld_capabilities())
            }
            Self::Google { config, .. } => {
                crate::consent::opened(&config.capabilities(), &config.withheld_capabilities())
            }
            Self::Imap { .. } | Self::Jmap { .. } => self.capabilities(),
        }
    }

    /// A newly registered IMAP account, with connections of its own: nothing is connected until
    /// the first dial or watch.
    pub(crate) fn imap_account(
        config: AccountConfig,
        tokens: Option<Arc<GraphTokenSource>>,
    ) -> Self {
        Self::Imap {
            config,
            connections: ImapConnections::new(),
            tokens,
        }
    }

    /// The IMAP config, connections and token source, or `None` for a Microsoft/JMAP account and
    /// for an IMAP account not used for mail: so an IMAP-only path (an `IDLE` watch) can skip
    /// entries with no mailbox to watch.
    ///
    /// They travel together because every IMAP dial needs all three: a watch that took the
    /// config alone would authenticate an OAuth account with nothing at all.
    pub(crate) fn imap(&self) -> Option<ImapParts<'_>> {
        match self {
            Self::Imap {
                config,
                connections,
                tokens,
            } if config
                .capabilities()
                .contains(mailcal_account::Capability::Mail) =>
            {
                Some((config, connections, tokens.as_ref()))
            }
            Self::Imap { .. } => None,
            Self::Microsoft { .. } | Self::Google { .. } | Self::Jmap { .. } => None,
        }
    }
}
