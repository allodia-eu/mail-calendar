//! What the binding layer remembers about a connected account, so it can reconnect one.
//!
//! Split from `lib.rs`, which is the FFI object and its lifecycle: this is a value type three
//! other modules read (`account_registry`, `token_sink`, `boot`). It is also the only layer that
//! knows which protocol a `dyn Provider` actually speaks, which is why the "what kind of account
//! is this" answers live on it.

use std::sync::Arc;

use mailcal_account::{
    AccountConfig, GoogleConfig, GraphTokenSource, JmapAccountConfig, MicrosoftConfig,
};

/// One connected account's re-connection state, kept so the on-demand [`HostConnector`]
/// and a sync-depth change can re-open a provider for any of its folders after the fact.
/// An IMAP account carries its config; a Microsoft account carries its config plus the
/// shared, self-refreshing [`GraphTokenSource`] every one of its folder providers uses.
/// Both hold credentials in memory only (never logged; their `Debug` redacts secrets).
#[derive(Debug)]
pub(crate) enum ConnectedAccount {
    /// An IMAP/SMTP/CalDAV (password) account.
    Imap(AccountConfig),
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
            Self::Imap(_) => "imap",
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
            Self::Imap(_) => mailcal_app::Protocol::Imap,
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
            Self::Imap(_) => crate::AccountProvider::Password,
            Self::Microsoft { .. } => crate::AccountProvider::Microsoft,
            Self::Google { .. } => crate::AccountProvider::Google,
            Self::Jmap { config, .. } if config.is_oauth() => crate::AccountProvider::JmapOauth,
            Self::Jmap { .. } => crate::AccountProvider::Jmap,
        }
    }

    /// The IMAP config, or `None` for a Microsoft/JMAP account: so an IMAP-only path
    /// (an `IDLE` watch) can skip non-IMAP entries.
    pub(crate) fn imap(&self) -> Option<&AccountConfig> {
        match self {
            Self::Imap(config) => Some(config),
            Self::Microsoft { .. } | Self::Google { .. } | Self::Jmap { .. } => None,
        }
    }
}
