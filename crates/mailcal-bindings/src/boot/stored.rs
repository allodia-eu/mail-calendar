//! Preparing the host's **stored** accounts for boot: kind detection, and the offline construction
//! of one account's id, identity, and re-connection state.
//!
//! There is no connect here any more. There used to be: a whole second implementation of "open an
//! account of family X", used only by the headless background worker, which had to independently
//! get the registry ordering right and did not. Dialing now happens in exactly one place
//! ([`AccountDial`](crate::account_registry::AccountDial)), and this module's job is the part that
//! must come *before* it: parse the config, derive the id, build the account's **one** token
//! source, and hand back an entry for the registry.
//!
//! Split out of `boot.rs` to keep each file under the 500-line limit; no FFI macros live here.

use std::sync::Arc;

use engine_api::{AccountId, EmailAddress, Provider, TimeZoneId};
use mailcal_account::{CredentialOrigin, GraphTokenSource, TokenSink};
use mailcal_app::Account;

use crate::{BoxedAccount, ConnectedAccount, MailcalError};

/// Builds one stored account **offline**; its id, identity, and re-connection state derived from
/// the config with no network at all.
///
/// Called for every account in both boot modes, before anything dials: the interactive app lists
/// the placeholder and paints its cached mail immediately, and the headless worker dials it a
/// moment later. The account comes back with empty providers (exactly like a failed dial), ready
/// for the dial to fill in.
///
/// The token source it builds is the account's **only** one, and the entry it returns carries it. A
/// second source over the same refresh token would be two independent refreshers of one credential
/// (the replay a ratcheting server revokes a grant for) which is why the dial reuses this one
/// rather than building its own.
///
/// `origin` says whether this credential came out of the host's store or out of a sign-in that has
/// just replaced it, which decides whether the account's token state may be adopted from another
/// core in this process. See [`CredentialOrigin`]; getting it wrong is silent for an hour and then
/// permanent.
///
/// # Errors
///
/// Returns [`MailcalError`] only if the config cannot be loaded, its id cannot be derived, or its
/// token source cannot be built; all fatal for that one account, which then cannot be listed at
/// all.
pub(crate) fn prepare_stored_account(
    config_toml: &str,
    sink: &Arc<dyn TokenSink>,
    origin: CredentialOrigin,
) -> Result<PreparedAccount, MailcalError> {
    let (id, identity, connected) = if is_microsoft_toml(config_toml) {
        let config = mailcal_account::load_microsoft_str(config_toml)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let id = config
            .account_id()
            .map_err(|err| MailcalError::Engine(err.to_string()))?;
        let identity = config.identity();
        // The shared, self-refreshing token source builds without a live socket.
        let tokens = GraphTokenSource::new(&config, id.clone(), Some(Arc::clone(sink)), origin)
            .map_err(|err| MailcalError::Connect(err.to_string()))?;
        (id, identity, ConnectedAccount::Microsoft { config, tokens })
    } else if is_google_toml(config_toml) {
        let config = mailcal_account::load_google_str(config_toml)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let id = config
            .account_id()
            .map_err(|err| MailcalError::Engine(err.to_string()))?;
        let identity = config.identity();
        // The provider-neutral token source builds without a live socket.
        let tokens = mailcal_account::google_token_source(
            &config,
            id.clone(),
            Some(Arc::clone(sink)),
            origin,
        )
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
        (id, identity, ConnectedAccount::Google { config, tokens })
    } else if is_jmap_toml(config_toml) {
        let config = mailcal_account::load_jmap_str(config_toml)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let id = config
            .account_id()
            .map_err(|err| MailcalError::Engine(err.to_string()))?;
        let identity = config.identity();
        // An OAuth JMAP account's token source builds without a live socket; a stored-secret
        // account has nothing to refresh and gets none.
        let tokens = jmap_tokens(&config, &id, sink, origin)?;
        (id, identity, ConnectedAccount::Jmap { config, tokens })
    } else {
        let config = mailcal_account::load_str(config_toml)
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let id = config
            .account_id()
            .map_err(|err| MailcalError::Engine(err.to_string()))?;
        let identity = EmailAddress::new(config.imap.username.clone());
        (id, identity, ConnectedAccount::Imap(config))
    };
    Ok(PreparedAccount {
        account: Account {
            id,
            providers: Vec::new(),
            calendar_providers: Vec::new(),
            contact_providers: Vec::new(),
            identity,
        },
        connected,
    })
}

/// One stored account prepared offline: the provider-less placeholder [`BoxedAccount`] the app
/// lists, and the [`ConnectedAccount`] entry the registry takes.
pub(crate) struct PreparedAccount {
    pub(crate) account: BoxedAccount,
    pub(crate) connected: ConnectedAccount,
}

/// Whether a stored config TOML is a Microsoft (OAuth) account; it parses as a `[microsoft]`
/// config. An IMAP config has no such section and fails this parse.
fn is_microsoft_toml(config_toml: &str) -> bool {
    mailcal_account::load_microsoft_str(config_toml).is_ok()
}

/// Whether a stored config TOML is a Google (OAuth) account; it parses as a `[google]` config. The
/// kinds are disjoint (each names a distinct top-level section), so a Microsoft / IMAP / JMAP
/// config has no `[google]` section and fails this parse.
fn is_google_toml(config_toml: &str) -> bool {
    mailcal_account::load_google_str(config_toml).is_ok()
}

/// Whether a stored config TOML is a JMAP account; it parses as a `[jmap]` config. The four kinds
/// are disjoint: an IMAP (`[imap]`), Microsoft (`[microsoft]`), or Google (`[google]`) config has
/// no `[jmap]` section and fails this parse, and vice versa.
pub(crate) fn is_jmap_toml(config_toml: &str) -> bool {
    mailcal_account::load_jmap_str(config_toml).is_ok()
}

/// Binds a Microsoft account's Graph calendar provider (its default calendar); shared by the dial
/// and the post-OAuth add path. A calendar-connect failure is **non-fatal**: mail comes up with an
/// empty agenda rather than failing the whole account.
///
/// Returns the (possibly empty) providers **and** whether the failure was a *scope-denied* `403`;
/// i.e. the account's OAuth grant predates the `Calendars.ReadWrite` scope, so the user must
/// **re-authenticate** to enable calendar (as opposed to a transient failure, which just retries on
/// the next sync). The caller records that as a per-account re-consent prompt.
pub(crate) async fn connect_graph_calendars(
    id: &AccountId,
    tokens: Arc<GraphTokenSource>,
    display_zone: TimeZoneId,
) -> (Vec<Box<dyn Provider>>, bool) {
    match mailcal_account::connect_graph_calendar_providers(id, tokens, display_zone).await {
        Ok(providers) => (providers, false),
        Err(mailcal_account::AccountError::CalendarAccessDenied(detail)) => {
            log::warn!("graph: calendar access denied; re-authentication needed: {detail}");
            (Vec::new(), true)
        }
        Err(err) => {
            log::warn!("graph: calendar connect failed, mail only: {err}");
            (Vec::new(), false)
        }
    }
}

/// The shared, self-refreshing token source for an **OAuth** JMAP account, or `None` for a
/// stored-secret one (which has nothing to refresh). Built without a live socket.
///
/// # Errors
///
/// Returns [`MailcalError::Connect`] if the OAuth HTTP client cannot be built; fatal for that
/// account, exactly as it is for a Microsoft one.
pub(super) fn jmap_tokens(
    config: &mailcal_account::JmapAccountConfig,
    id: &engine_api::AccountId,
    sink: &Arc<dyn TokenSink>,
    origin: CredentialOrigin,
) -> Result<Option<Arc<GraphTokenSource>>, MailcalError> {
    let Some(grant) = config.oauth.as_ref() else {
        return Ok(None);
    };
    mailcal_account::jmap_token_source(grant, id.clone(), Some(Arc::clone(sink)), origin)
        .map(Some)
        .map_err(|err| MailcalError::Connect(err.to_string()))
}
