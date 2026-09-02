//! `mailcal-account`; load an account's connection config and build the concrete
//! providers it drives.
//!
//! It bridges the engine's provider adapters (`provider-imap`, `provider-caldav`)
//! and the app: a host reads a TOML config (endpoints +
//! credentials) and this crate turns it into a connected
//! [`engine_provider::Provider`] the app syncs through. The config carries secrets,
//! so it stays out of logs (see [`Secret`]) and out of version control: a real host
//! uses the OS keychain; the `probe` binary reads a gitignored file outside the repo.

use std::{fmt, sync::Arc};

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
mod jmap;
mod log_handle;
mod microsoft;
mod preferences;
mod reconnect;
mod recurrence_shape;
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
use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId},
    mail::MailboxRole,
    sync::SyncUpdate,
};
use engine_provider::{Provider, ProviderError, Watch};
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
pub use jmap::{
    JmapAccountConfig, JmapOAuth, JmapSetup, build_jmap_config_toml,
    connect_jmap_calendar_providers, connect_jmap_contact_providers, connect_jmap_folder,
    connect_jmap_mail_providers, jmap_base_url, jmap_token_source, load_jmap_str,
};
pub use log_handle::account_log_handle;
pub use microsoft::{MicrosoftConfig, fetch_primary_address, load_microsoft_str};
pub use preferences::{
    AccountSyncSettings, Appearance, CalendarLayout, CalendarPrefs, DEFAULT_POLL_INTERVAL,
    DEFAULT_VISIBLE_HOURS, DefaultCalendar, EffectiveSync, MAX_PUSH_FOLDERS, MAX_VISIBLE_HOURS,
    MESSAGE_SIZE_LIMITS_MB, MIN_VISIBLE_HOURS, MessageGrouping, MessageSizeLimit, POLL_INTERVALS,
    Preferences, QuoteStyle, ReplyFallback, SYNC_DEPTHS, SwipeAction, SyncDepth, SyncStrategy,
    TimeFormat, WeekStart, cap_push_folders, clamp_visible_hours, effective, load_preferences,
    preferences_path, save_preferences, snap_poll_interval,
};
use provider_caldav::{CalDavConfig, CalDavProvider, Credentials};
use provider_imap::{DEFAULT_IDLE_KEEPALIVE, ImapConfig, ImapError, ImapProvider, ImapWatcher};
use reconnect::{ReconnectingImapProvider, Redial};
pub use recurrence_shape::{
    EventRecurrence, RecurrenceChange, RecurrenceDay, RecurrenceEnd, RecurrenceFrequency,
    RecurrenceWeekday, SimpleRecurrence, describe_recurrence, recurrence_rule_of,
    undrawable_reason,
};
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

/// Applies the optional sync-depth cutoff to an IMAP config: a `Some(date)` bounds mail
/// sync to messages delivered on or after it (`ImapConfig::with_since`); `None` syncs the
/// whole mailbox. One place so every connect path windows consistently.
fn windowed(config: ImapConfig, since: Option<time::Date>) -> ImapConfig {
    match since {
        Some(date) => config.with_since(date),
        None => config,
    }
}

/// Connects to one IMAP `mailbox` of `account` over a certificate-verifying TLS
/// connector (Mozilla roots), bounding mail sync to `since` (the sync-depth cutoff;
/// `None` for all mail), returning the provider boxed for the app to sync. Used by the
/// host's on-demand "sync the folder you open" path.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id or the connection/login
/// fails.
pub async fn connect_imap_mailbox(
    account: &AccountConfig,
    mailbox: &str,
    since: Option<time::Date>,
) -> Result<Box<dyn Provider>, AccountError> {
    let mailbox =
        MailboxId::try_from(mailbox).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    let config = windowed(account.imap_config(), since);
    let tls = account_tls()?;
    let provider = ImapProvider::connect(&config, tls.connector(), mailbox.clone()).await?;
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let redial = make_imap_redial(config, mailbox.clone(), tls);
    Ok(Box::new(ReconnectingImapProvider::adopt(
        provider, mailbox, redial,
    )))
}

/// Builds the re-dial closure a [`ReconnectingImapProvider`] uses to rebuild a dropped IMAP
/// session: it re-runs [`ImapProvider::connect`] with the same **windowed** config and bound
/// `mailbox`, so a reconnect re-applies the sync-depth window and re-selects the same folder.
/// The shared TLS config is captured by value and cloned per dial (an `Arc` bump), keeping every
/// reconnect on the account's selected trust policy.
fn make_imap_redial(
    config: ImapConfig,
    mailbox: MailboxId,
    tls: engine_tls::TlsClientConfig,
) -> Redial {
    Box::new(move || {
        let config = config.clone();
        let mailbox = mailbox.clone();
        let tls = tls.clone();
        Box::pin(async move {
            ImapProvider::connect(&config, tls.connector(), mailbox)
                .await
                .map(|provider| Arc::new(provider) as Arc<dyn Provider>)
                .map_err(ProviderError::from)
        })
    })
}

/// Opens a standing IMAP `IDLE` watch on one `mailbox` of `account`, over the same
/// certificate-verifying TLS connector as the sync providers, returning it boxed behind
/// the engine's neutral [`Watch`] contract. The watch is a **separate connection** from
/// the sync provider (a connection in `IDLE` cannot `FETCH`), so the host drives this from
/// its own task and runs the mailbox's sync on the provider when it reports
/// [`WatchEvent`](engine_provider::WatchEvent)`::Changed`. No sync-depth window applies; a
/// watch carries no data, only the signal to sync (the sync itself windows). Used by the
/// host's "receive emails as they come in" (push) path; the parallel of
/// [`connect_imap_mailbox`] for watching rather than syncing.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id, the connection/login fails, or
/// the server does not advertise `IDLE` (the host then falls back to polling).
pub async fn connect_imap_watcher(
    account: &AccountConfig,
    mailbox: &str,
) -> Result<Box<dyn Watch>, AccountError> {
    let mailbox =
        MailboxId::try_from(mailbox).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    // A watch carries no mail, so it is never windowed: the sync it triggers applies the
    // sync-depth cutoff. The keep-alive is the engine's RFC 2177-safe default (clamped by
    // the adapter); a shorter mobile interval is a future per-platform refinement.
    let tls = account_tls()?;
    let watcher = ImapWatcher::connect(
        &account.imap_config(),
        tls.connector(),
        mailbox,
        DEFAULT_IDLE_KEEPALIVE,
    )
    .await
    .map_err(|err| AccountError::Watch(err.to_string()))?;
    Ok(Box::new(watcher))
}

/// The non-INBOX folder roles the app eagerly binds a provider to at startup, so their
/// messages sync and render up front (Sent threads a reply with its original; Trash shows
/// deleted mail; Drafts / Archive / Junk are the other folders a user navigates first).
/// Folder names are server-specific, so each is resolved by its SPECIAL-USE role. Any
/// **other** folder (a server that doesn't tag Archive, or a custom folder) syncs **on
/// demand** when the user opens it, via the host's `MailboxConnector` +
/// [`connect_imap_mailbox`]: so no folder is permanently empty.
const SYNCED_ROLES: &[MailboxRole] = &[
    MailboxRole::Sent,
    MailboxRole::Drafts,
    MailboxRole::Trash,
    MailboxRole::Archive,
    MailboxRole::Junk,
];

/// Connects the IMAP providers the app syncs: the INBOX plus every folder carrying one
/// of the `SYNCED_ROLES` (Sent, Drafts, Trash, Archive, Junk), each resolved by its
/// role (its name is server-specific) from the account's folder list. So sent mail
/// threads with its original and the Trash/Drafts/etc. folders render their contents.
/// Returns one boxed provider per bound mailbox (just the INBOX when none of the roles
/// exist).
///
/// # Errors
///
/// Returns [`AccountError`] if a connection/login fails or the folder list cannot be
/// fetched.
pub async fn connect_mail_providers(
    account: &AccountConfig,
    account_id: &AccountId,
    since: Option<time::Date>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let config = windowed(account.imap_config(), since);
    let tls = account_tls()?;
    let inbox_id =
        MailboxId::try_from("INBOX").map_err(|err| AccountError::Mailbox(err.to_string()))?;
    // The account's first login, and the only one that can prove the stored password wrong: a
    // refusal here has nothing to contradict it, while one in the folder loop below has the
    // success of this connect (see `from_first_imap_login`).
    let inbox = ImapProvider::connect(&config, tls.connector(), inbox_id.clone())
        .await
        .map_err(AccountError::from_first_imap_login)?;
    let inbox: Arc<dyn Provider> = Arc::new(inbox);

    // Enumerate folders to find the role mailboxes (their names vary by server).
    let listing = inbox
        .sync_mailboxes(account_id, None)
        .await
        .map_err(|err| AccountError::MailboxList(err.to_string()))?;
    let folders = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    let role_folders: Vec<MailboxId> = folders
        .into_iter()
        .filter(|mailbox| {
            mailbox
                .role
                .as_ref()
                .is_some_and(|role| SYNCED_ROLES.contains(role))
        })
        .map(|mailbox| mailbox.id)
        .collect();

    // Each provider self-heals: on a dropped connection it re-dials a fresh session and
    // retries, so Refresh / opening a message recovers without an app restart.
    let mut providers: Vec<Box<dyn Provider>> = vec![Box::new(ReconnectingImapProvider::adopt(
        inbox,
        inbox_id.clone(),
        make_imap_redial(config.clone(), inbox_id, tls.clone()),
    ))];
    for id in role_folders {
        let provider = ImapProvider::connect(&config, tls.connector(), id.clone()).await?;
        let provider: Arc<dyn Provider> = Arc::new(provider);
        let redial = make_imap_redial(config.clone(), id.clone(), tls.clone());
        providers.push(Box::new(ReconnectingImapProvider::adopt(
            provider, id, redial,
        )));
    }
    Ok(providers)
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
pub async fn connect_caldav(account: &AccountConfig) -> Result<Box<dyn Provider>, AccountError> {
    let caldav = account.caldav.as_ref().ok_or(AccountError::NoCalDav)?;
    let tls = account_tls()?;
    let config = CalDavConfig::new(
        // Tolerate a stored bare host (a scheme-less base URL from an earlier setup) by
        // defaulting it to https:// here too, so existing configs connect without re-entry.
        normalize_caldav_base_url(&caldav.base_url),
        Credentials::Basic {
            username: caldav.username.clone(),
            password: caldav.password.expose().to_owned(),
        },
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
            connect_caldav(&config).await,
            Err(AccountError::NoCalDav)
        ));
    }
}
