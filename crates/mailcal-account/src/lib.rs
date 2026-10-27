//! `mailcal-account`; load an account's connection config and build the concrete
//! providers it drives.
//!
//! It bridges the engine's provider adapters (`provider-imap`, `provider-caldav`)
//! and the app: a host reads a TOML config (endpoints +
//! credentials) and this crate turns it into a connected
//! [`engine_provider::Provider`] the app syncs through. The config carries secrets,
//! so it stays out of logs (see [`Secret`]) and out of version control: a real host
//! uses the OS keychain; the `probe` binary reads a gitignored file outside the repo.

mod account_shape;
mod affiliation;
mod autodetect;
mod calendar;
mod calendar_drag;
mod certificate;
mod config;
mod config_dav;
mod config_edit;
mod config_split;
mod connect_log;
mod consent;
mod contacts;
mod contacts_edit;
mod dav_starts;
mod delegate_info;
/// Dev-only extra-CA trust for the local test harness; compiled out of production builds (present
/// in a debug build, or a release build with the `dev-harness` feature for the Android dev loop).
#[cfg(any(debug_assertions, feature = "dev-harness"))]
mod dev_tls;
mod error;
mod event_detail;
mod google;
mod graph;
mod imap;
mod imap_auth;
mod imap_credentials;
mod jmap;
mod log_handle;
mod microsoft;
mod oauth_grant;
mod pass_folders;
mod preferences;
mod reconnect;
mod recurrence_shape;
mod repeat_draft;
mod repeat_summary;
mod series_warning;
mod setup;
mod setup_choices;
mod signatures;
mod stream_redial;
mod throttle;
mod tls;

pub use account_shape::{AccountLinks, AccountShape, Capabilities, Capability};
pub use autodetect::{MissReason, OauthRoutes, ServerSummary, SetupRecommendation, recommend};
pub use calendar::{
    EventEdit, attach_meeting, build_event_deletion, build_event_draft, build_event_patch,
};
pub use calendar_drag::{
    EventDrag, EventEdge, apply_event_drag, names_an_occurrence, occurrence_local,
    occurrence_wall_clock, stored_occurrence,
};
pub use certificate::{CertificateException, RejectedCertificate, format_fingerprint};
pub use config::{
    AccountConfig, CalDavAccount, ConfigError, ConnectionSecurity, ImapAccount, Secret,
    SmtpAccount, default_path, load, load_str,
};
pub use config_dav::{CardDavAccount, grant_reaches};
pub use config_edit::{EditedAccount, EndpointEdit};
pub use consent::{requested_scopes, withheld};
pub use contacts::{connect_carddav_contact_providers, connect_carddav_learning};
pub use contacts_edit::{ContactEdit, build_contact_draft, build_contact_patch};
pub use dav_starts::{DavLearned, DavStarts, DavStartsFile, Learned};
use engine_provider::Provider;
pub use error::AccountError;
pub use event_detail::{DetailOccurrence, EventDetail, InviteeEditability, project_event_detail};
pub use google::{
    GoogleConfig, connect_google_calendar_provider, connect_google_contact_providers,
    connect_google_folder, connect_google_mail_providers, fetch_google_primary_address,
    google_token_source, load_google_str,
};
pub use graph::{
    CredentialOrigin, GraphTokenSource, TokenSink, connect_graph_calendar_provider,
    connect_graph_contact_providers, connect_graph_folder, connect_graph_mail_providers,
    google_affiliation, graph_affiliation,
};
pub use imap::{
    ImapConnections, connect_imap_mailbox, connect_imap_watcher, connect_mail_providers,
};
pub use imap_auth::{
    ImapAuth, ImapAuthQuery, decide_imap_auth, imap_issuer, imap_offers_password, log_smtp_auth,
};
pub use imap_credentials::{ImapTokens, OAuthCredentialSource, imap_credential_source};
pub use jmap::{
    JmapAccountConfig, JmapSetup, build_jmap_config_toml, connect_jmap_calendar_provider,
    connect_jmap_contact_providers, connect_jmap_folder, connect_jmap_mail_providers,
    jmap_base_url, load_jmap_str,
};
pub use log_handle::account_log_handle;
pub use microsoft::{MicrosoftConfig, fetch_primary_address, load_microsoft_str};
pub use oauth_grant::{OAuthGrant, oauth_token_source};
pub use pass_folders::pass_syncs;
pub use preferences::{
    AccountSyncSettings, Appearance, CalendarLayout, CalendarPrefs, DEFAULT_POLL_INTERVAL,
    DEFAULT_VISIBLE_HOURS, DefaultCalendar, EffectiveSync, MAX_PUSH_FOLDERS, MAX_SENDER_NAME_CHARS,
    MAX_VISIBLE_HOURS, MESSAGE_SIZE_LIMITS_MB, MIN_VISIBLE_HOURS, MessageGrouping,
    MessageSizeLimit, POLL_INTERVALS, Preferences, QuoteStyle, ReplyFallback, SYNC_DEPTHS,
    SwipeAction, SyncDepth, SyncStrategy, TimeFormat, WeekStart, cap_push_folders,
    clamp_visible_hours, effective, load_preferences, preferences_path, sanitize_sender_name,
    save_preferences, snap_poll_interval,
};
use provider_caldav::{CalDavConfig, CalDavProvider};
pub use recurrence_shape::{
    EventRecurrence, RecurrenceChange, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency,
    RecurrenceWeekday, SimpleRecurrence, describe_recurrence, recurrence_rule_of,
    undrawable_reason,
};
pub use repeat_draft::{RepeatDraft, recurrence_change_of, repeat_draft_of, rule_from_draft};
pub use repeat_summary::{RepeatRhythm, RepeatStop, RepeatSummary, summarize_repeat};
pub use series_warning::{
    SeriesEditTouches, SeriesEditWarning, series_edit_touches, series_edit_warning,
};
pub use setup::{
    AccountSetup, SetupCredential, build_config_toml, imap_default_port, smtp_default_port,
};
pub use setup_choices::{SetupChoice, google_choices, microsoft_choices, standards_choices};
pub use signatures::{
    AccountSignatureAssignment, SignatureId, SignatureSlot, Signatures, StoredSignature,
    load_signatures, save_signatures, signatures_path,
};
pub use tls::setup_trust_policy;

pub use crate::setup::normalize_caldav_base_url;
use crate::tls::account_tls;

/// Connects to the CalDAV endpoint of `account`, discovering the calendar home and
/// binding to the calendar to sync events from, returning the provider boxed for the
/// app to sync.
///
/// When the config names a `calendar`, binds to it directly. Otherwise discovers the
/// account's calendars and binds to the first one: real servers (Soverin/SabreDAV)
/// name calendars with server-generated ids rather than a literal `default`, so a
/// host that hasn't picked a calendar must discover an actual collection rather than
/// guess its name. Authenticates with HTTP Basic (the common CalDAV case); the
/// transport verifies the server certificate via the account's shared TLS policy.
///
/// # Errors
///
/// Returns [`AccountError`] if `account` has no `[caldav]` section, the
/// connection/discovery fails, or no calendar collection is discovered.
pub async fn connect_caldav(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
) -> Result<Box<dyn Provider>, AccountError> {
    connect_caldav_learning(account, tokens)
        .await
        .map(|connected| connected.providers)
}

/// The `User-Agent` every engine adapter this crate opens sends ([`mailcal_user_agent`]).
pub(crate) fn user_agent() -> engine_api::UserAgent {
    static USER_AGENT: std::sync::LazyLock<engine_api::UserAgent> =
        std::sync::LazyLock::new(|| {
            engine_api::UserAgent::new(mailcal_user_agent::PRODUCT, mailcal_user_agent::VERSION)
                .expect("the build names the product and version as tokens")
        });
    USER_AGENT.clone()
}

/// A DAV connect's providers, and the principal the server named on the way: what a host
/// stores for the next connect to start from ([`DavStarts`]).
pub struct DavConnected<T> {
    /// The connected providers.
    pub providers: T,
    /// The principal URL the server named, absolute, when it named one.
    pub principal: Option<String>,
}

impl<T> DavConnected<T> {
    /// `self` from a connect that started at the stored principal `start` and succeeded there: a
    /// server that named no principal on the way said nothing against it, so it stays the start.
    /// Only a connect that fell back from it to the configured server forgets it.
    #[must_use]
    pub(crate) fn started_at(mut self, start: &str) -> Self {
        self.principal.get_or_insert_with(|| start.to_owned());
        self
    }
}

impl<T> std::fmt::Debug for DavConnected<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The providers hold connection handles; the principal is what the result is about.
        f.debug_struct("DavConnected")
            .field("principal", &self.principal)
            .finish_non_exhaustive()
    }
}

/// [`connect_caldav`], reporting the principal the server named. It starts at the principal the
/// last connect stored ([`DavStarts::calendar`]) when there is one, and from the configured
/// server when there is none or the server says it is gone, so a principal that moved within the
/// server is found again without looking up DNS.
///
/// # Errors
///
/// As [`connect_caldav`].
pub async fn connect_caldav_learning(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
) -> Result<DavConnected<Box<dyn Provider>>, AccountError> {
    if let Some(start) = account.dav_starts.calendar.as_deref() {
        match connect_caldav_at(account, tokens, Some(start)).await {
            Err(err) if err.is_gone() => {
                log::info!(
                    "caldav: the stored principal is gone; starting from the configured server"
                );
            }
            connected => return connected.map(|connected| connected.started_at(start)),
        }
    }
    connect_caldav_at(account, tokens, None).await
}

/// One CalDAV connect, starting at `start` when given and at the configured server otherwise.
async fn connect_caldav_at(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
    start: Option<&str>,
) -> Result<DavConnected<Box<dyn Provider>>, AccountError> {
    let endpoint = account.caldav_endpoint().ok_or(AccountError::NoCalDav)?;
    let auth =
        config_dav::dav_credentials(account, endpoint, mailcal_oauth::DavUse::Calendars, tokens)
            .await?;
    let caldav = account.caldav.as_ref().ok_or(AccountError::NoCalDav)?;
    let tls = account_tls(account)?;
    let mut config = CalDavConfig::new(
        // Tolerate a stored bare host (a scheme-less base URL from an earlier setup) by
        // defaulting it to https:// here too, so existing configs connect without re-entry.
        normalize_caldav_base_url(&caldav.base_url),
        auth.credentials,
        crate::user_agent(),
    );
    if let Some(hosts) = auth.hosts {
        config = config.with_credential_hosts(hosts);
    }
    if let Some(start) = start {
        config = config.with_discovery_path(start);
    }
    let config = config
    // A clone shares the record of what it refused, so a refusal reads back from `tls`.
    .with_tls(tls.clone())
    // Ungated, for the reason `connect_carddav_contact_providers` gives: no DAV adapter
    // states a ceiling yet.
    .with_retry(throttle::ungated_retry())
    .with_connect_observer(connect_log::connect_logger("caldav"));
    let provider = CalDavProvider::connect(config)
        .await
        .map_err(AccountError::from_first_dav_connect)
        .map_err(|err| err.over_tls(&tls))?;
    let principal = provider.principal_url().map(str::to_owned);
    Ok(DavConnected {
        providers: Box::new(provider),
        principal,
    })
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;

#[cfg(test)]
mod tests {
    use super::{AccountConfig, AccountError, connect_caldav};

    /// The engine's adapters send the same header as the requests this application makes itself.
    #[test]
    fn the_engine_names_this_application_as_its_own_requests_do() {
        assert_eq!(super::user_agent().as_str(), mailcal_user_agent::HEADER);
    }

    #[tokio::test]
    async fn connect_caldav_without_a_caldav_section_is_an_error() {
        // No network: the missing-endpoint check short-circuits before connecting.
        let config: AccountConfig = toml::from_str(
            "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"p\"\n",
        )
        .expect("valid config");
        assert!(matches!(
            connect_caldav(&config, None).await,
            Err(AccountError::NoCalDav)
        ));
    }

    /// A connect that succeeded at the stored principal keeps it when the server named none, so
    /// the next connect does not start from the configured server again.
    #[test]
    fn a_connect_from_the_stored_principal_keeps_it_when_the_server_names_none() {
        let start = "https://dav.example.net/principals/ada/";
        let unnamed = super::DavConnected {
            providers: (),
            principal: None,
        };
        assert_eq!(unnamed.started_at(start).principal.as_deref(), Some(start));

        let renamed = super::DavConnected {
            providers: (),
            principal: Some("https://dav.example.net/principals/ada2/".to_owned()),
        };
        assert_eq!(
            renamed.started_at(start).principal.as_deref(),
            Some("https://dav.example.net/principals/ada2/")
        );
    }
}
