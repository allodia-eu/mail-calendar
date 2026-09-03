//! `mailcal-account`; load an account's connection config and build the concrete
//! providers it drives.
//!
//! It bridges the engine's provider adapters (`provider-imap`, `provider-caldav`)
//! and the app: a host reads a TOML config (endpoints +
//! credentials) and this crate turns it into a connected
//! [`engine_provider::Provider`] the app syncs through. The config carries secrets,
//! so it stays out of logs (see [`Secret`]) and out of version control: a real host
//! uses the OS keychain; the `probe` binary reads a gitignored file outside the repo.

use std::fmt;

mod autodetect;
mod calendar;
mod calendar_drag;
mod config;
mod connect_log;
mod contacts;
mod contacts_edit;
mod delegate_info;
/// Dev-only extra-CA trust for the local test harness; compiled out of production builds (present
/// in a debug build, or a release build with the `dev-harness` feature for the Android dev loop).
#[cfg(any(debug_assertions, feature = "dev-harness"))]
mod dev_tls;
mod event_detail;
mod google;
mod graph;
mod imap;
mod jmap;
mod log_handle;
mod microsoft;
mod oauth_grant;
mod preferences;
mod reconnect;
mod recurrence_shape;
mod repeat_draft;
mod repeat_summary;
mod series_warning;
mod setup;
mod signatures;
mod throttle;
mod tls;

pub use autodetect::{MissReason, OauthRoutes, ServerSummary, SetupRecommendation, recommend};
pub use calendar::{EventEdit, build_event_deletion, build_event_draft, build_event_patch};
pub use calendar_drag::{
    EventDrag, EventEdge, apply_event_drag, names_an_occurrence, occurrence_local,
    occurrence_wall_clock, stored_occurrence,
};
pub use config::{
    AccountConfig, CalDavAccount, ConfigError, ConnectionSecurity, ImapAccount, Secret,
    SmtpAccount, default_path, load, load_str,
};
pub use contacts::connect_carddav_contact_providers;
pub use contacts_edit::{ContactEdit, build_contact_draft, build_contact_patch};
use engine_core::{error::FailureClass, ids::AccountId, sync::SyncUpdate};
use engine_provider::Provider;
pub use event_detail::{DetailOccurrence, EventDetail, project_event_detail};
pub use google::{
    GoogleConfig, connect_google_calendar_providers, connect_google_folder,
    connect_google_mail_providers, fetch_google_primary_address, google_token_source,
    load_google_str,
};
pub use graph::{
    CredentialOrigin, GraphTokenSource, TokenSink, connect_graph_calendar_providers,
    connect_graph_folder, connect_graph_mail_providers,
};
pub use imap::{
    ImapTokens, connect_imap_mailbox, connect_imap_watcher, connect_mail_providers,
    imap_credentials,
};
pub use jmap::{
    JmapAccountConfig, JmapSetup, build_jmap_config_toml, connect_jmap_calendar_providers,
    connect_jmap_contact_providers, connect_jmap_folder, connect_jmap_mail_providers,
    jmap_base_url, load_jmap_str,
};
pub use log_handle::account_log_handle;
pub use microsoft::{MicrosoftConfig, fetch_primary_address, load_microsoft_str};
pub use oauth_grant::{OAuthGrant, oauth_token_source};
pub use preferences::{
    AccountSyncSettings, Appearance, CalendarLayout, CalendarPrefs, DEFAULT_POLL_INTERVAL,
    DEFAULT_VISIBLE_HOURS, DefaultCalendar, EffectiveSync, MAX_PUSH_FOLDERS, MAX_VISIBLE_HOURS,
    MESSAGE_SIZE_LIMITS_MB, MIN_VISIBLE_HOURS, MessageGrouping, MessageSizeLimit, POLL_INTERVALS,
    Preferences, QuoteStyle, ReplyFallback, SYNC_DEPTHS, SwipeAction, SyncDepth, SyncStrategy,
    TimeFormat, WeekStart, cap_push_folders, clamp_visible_hours, effective, load_preferences,
    preferences_path, save_preferences, snap_poll_interval,
};
use provider_caldav::{CalDavConfig, CalDavProvider, Credentials};
use provider_imap::ImapError;
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
pub use setup::{AccountSetup, build_config_toml};
pub use signatures::{
    AccountSignatureAssignment, SignatureId, SignatureSlot, Signatures, StoredSignature,
    load_signatures, save_signatures, signatures_path,
};

use crate::{setup::normalize_caldav_base_url, tls::account_tls};

/// The CalDAV credential for `account`: HTTP Basic from the stored password, or the mail
/// grant's bearer token when the account signs in with OAuth.
///
/// A discovered calendar rides on the mail account's own credential (`docs/mail-oauth.md`),
/// so an OAuth account has no password to reuse here and presents the same token instead:
/// the `calendars` scope is requested at sign-in precisely so this works. The token is minted
/// per connect, like the mail one.
///
/// # Errors
///
/// Returns [`AccountError::SigninRejected`] if the grant no longer mints a token, or
/// [`AccountError::NoCalDav`] if the account carries no CalDAV endpoint.
pub(crate) async fn caldav_credentials(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
) -> Result<Credentials, AccountError> {
    let caldav = account.caldav.as_ref().ok_or(AccountError::NoCalDav)?;
    if account.is_oauth() {
        let tokens = tokens.ok_or_else(|| {
            AccountError::Jmap(
                "this account signs in with OAuth but was connected without a token source"
                    .to_owned(),
            )
        })?;
        return Ok(Credentials::Bearer(tokens.access_token().await?));
    }
    let password = caldav
        .password
        .as_ref()
        .ok_or_else(|| AccountError::CalDavDiscovery("no calendar credential stored".to_owned()))?;
    Ok(Credentials::Basic {
        username: caldav.username.clone(),
        password: password.expose().to_owned(),
    })
}

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
    let credentials = caldav_credentials(account, tokens).await?;
    let caldav = account.caldav.as_ref().ok_or(AccountError::NoCalDav)?;
    let tls = account_tls()?;
    let config = CalDavConfig::new(
        // Tolerate a stored bare host (a scheme-less base URL from an earlier setup) by
        // defaulting it to https:// here too, so existing configs connect without re-entry.
        normalize_caldav_base_url(&caldav.base_url),
        credentials,
    )
    .with_tls(tls)
    .with_retry(throttle::account_retry())
    .with_connect_observer(connect_log::connect_logger("caldav"));
    let provider = match &caldav.calendar {
        Some(calendar) => CalDavProvider::connect(config.with_calendar(calendar.clone())).await?,
        None => connect_primary_calendar(config).await?,
    };
    Ok(Box::new(provider))
}

/// Connects and rebinds to the account's first discovered calendar, for a config
/// that did not name one (see [`connect_caldav`]).
async fn connect_primary_calendar(config: CalDavConfig) -> Result<CalDavProvider, AccountError> {
    let provider = CalDavProvider::connect(config).await?;
    // The account scopes the listing but not which collections come back, so a
    // placeholder id is fine here.
    let account =
        AccountId::try_from("caldav-discovery").map_err(AccountError::caldav_discovery)?;
    let listing = provider
        .sync_calendars(&account, None)
        .await
        .map_err(AccountError::caldav_discovery)?;
    let first = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects.into_iter().next(),
        SyncUpdate::Delta { changed, .. } => changed.into_iter().next(),
    };
    let calendar = first.ok_or(AccountError::NoCalendarDiscovered)?;
    provider
        .rebind(calendar.id.as_str())
        .map_err(AccountError::from)
}

/// An error building or connecting an account's providers.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    /// The mailbox id was not valid.
    #[error("invalid mailbox id: {0}")]
    Mailbox(String),
    /// Listing the account's folders failed (needed to find the Sent mailbox).
    #[error("listing mailboxes: {0}")]
    MailboxList(String),
    /// The IMAP connection or login failed.
    #[error("imap: {0}")]
    Imap(#[from] provider_imap::ImapError),
    /// Building the account's shared TLS policy failed.
    #[error("tls: {0}")]
    Tls(#[from] engine_tls::TlsError),
    /// A Microsoft Graph call failed (the `/me` address lookup, a token refresh, or a
    /// folder-list/connect error) while building a Microsoft account's providers.
    #[error("graph: {0}")]
    Graph(String),
    /// The server refused the account's **stored credential** outright: an OAuth refresh token
    /// that is revoked or expired (`invalid_grant`, `AADSTS700082`) and so mints no access token,
    /// or a password/API token answered with `[AUTHENTICATIONFAILED]` / `401`. Distinct from every
    /// per-family variant because it is the one failure a retry cannot fix and an outage badge
    /// misdescribes (the server *was* reached) so a caller prompts "sign in again"
    /// (`docs/provider-oauth.md` rule 12).
    ///
    /// Deliberately not named for a token: every family's refusal maps here, and which kind of
    /// credential the server refused changes nothing a caller does about it.
    #[error("sign-in rejected: {0}")]
    SigninRejected(String),
    /// A Google (Gmail/Calendar) call failed (the profile address lookup, a token refresh, or
    /// a calendar-list/connect error) while building or driving a Google account's providers.
    /// The Google parallel of [`AccountError::Graph`].
    #[error("google: {0}")]
    Google(String),
    /// The Graph **calendar** probe (`GET /me/calendars`) was refused with a `403`; the
    /// account's OAuth grant lacks the `Calendars.ReadWrite` scope (it was connected before
    /// calendar support, or consent was revoked). Distinct from a transient
    /// [`AccountError::Graph`] so a caller can prompt the user to **re-authenticate to grant
    /// calendar access** rather than badge a generic outage: mail is unaffected.
    #[error("calendar access denied (re-authentication needed): {0}")]
    CalendarAccessDenied(String),
    /// A JMAP call failed (session discovery, connect, or a sync/submission error)
    /// while building or driving a JMAP account's provider.
    #[error("jmap: {0}")]
    Jmap(String),
    /// Opening an IMAP `IDLE` watch failed (connect/login error, or the server does not
    /// advertise `IDLE`: the host falls back to polling).
    #[error("imap watch: {0}")]
    Watch(String),
    /// CalDAV was requested but the config has no `[caldav]` section.
    #[error("no caldav endpoint configured")]
    NoCalDav,
    /// The CalDAV connection or discovery failed.
    #[error("caldav: {0}")]
    CalDav(#[from] provider_caldav::CalDavError),
    /// Listing the account's calendars failed (no `calendar` was configured, so the
    /// connection had to discover one).
    #[error("caldav calendar discovery: {0}")]
    CalDavDiscovery(String),
    /// No calendar collection was discovered, and the config named none to bind to.
    #[error("no caldav calendar discovered (configure `calendar` in [caldav])")]
    NoCalendarDiscovered,
    /// Building a calendar event-write failed (a bad uid, time, or href).
    #[error("calendar write: {0}")]
    CalendarWrite(String),
    /// Building a contact write failed: the edit named nothing to file the card under, or
    /// carried a value that is not an email address.
    ///
    /// The message states the *shape* that was wrong and never quotes the value: a contact's
    /// values are content, and this reaches the diagnostic log (`docs/logging.md`).
    #[error("contact write: {0}")]
    ContactWrite(String),
}

impl AccountError {
    /// Wraps a calendar-discovery failure, keeping its message (the source types
    /// differ: a provider error or an invalid placeholder id).
    fn caldav_discovery(err: impl fmt::Display) -> Self {
        Self::CalDavDiscovery(err.to_string())
    }

    /// The verdict for the login that **first** presents an IMAP account's password in a dial;
    /// the only one whose refusal can mean the password itself is no good. An
    /// [authentication-class](FailureClass::Authentication) refusal becomes
    /// [`Self::SigninRejected`]; anything else keeps [`Self::Imap`].
    ///
    /// A later connection of the same dial deliberately does **not** come through here. The same
    /// password authenticated seconds earlier, so a refusal there is the server contradicting
    /// itself, and prompting for a new sign-in over it is the false prompt
    /// `docs/provider-oauth.md` rule 12 forbids; servers do refuse a valid credential.
    fn from_first_imap_login(err: ImapError) -> Self {
        if err.failure_class() == FailureClass::Authentication {
            Self::SigninRejected(err.to_string())
        } else {
            Self::Imap(err)
        }
    }

    /// The verdict for a JMAP connect, which presents the credential on **every** attempt (session
    /// discovery authenticates, so there is no first-then-folders sequence to corroborate against):
    /// an [authentication-class](FailureClass::Authentication) refusal: a `401` to a password, an
    /// API token or a bearer; becomes [`Self::SigninRejected`], anything else [`Self::Jmap`].
    pub(crate) fn from_jmap_connect(err: &provider_jmap::JmapError) -> Self {
        if err.failure_class() == FailureClass::Authentication {
            Self::SigninRejected(err.to_string())
        } else {
            Self::Jmap(err.to_string())
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;

#[cfg(test)]
mod tests {
    use super::{AccountConfig, AccountError, connect_caldav};

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
}
