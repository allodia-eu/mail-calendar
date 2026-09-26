//! `mailcal-account`; load an account's connection config and build the concrete
//! providers it drives.
//!
//! It bridges the engine's provider adapters (`provider-imap`, `provider-caldav`)
//! and the app: a host reads a TOML config (endpoints +
//! credentials) and this crate turns it into a connected
//! [`engine_provider::Provider`] the app syncs through. The config carries secrets,
//! so it stays out of logs (see [`Secret`]) and out of version control: a real host
//! uses the OS keychain; the `probe` binary reads a gitignored file outside the repo.

mod autodetect;
mod calendar;
mod calendar_drag;
mod certificate;
mod config;
mod connect_log;
mod contacts;
mod contacts_edit;
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
mod jmap;
mod log_handle;
mod microsoft;
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
pub use certificate::{CertificateException, RejectedCertificate, format_fingerprint};
pub use config::{
    AccountConfig, CalDavAccount, ConfigError, ConnectionSecurity, ImapAccount, Secret,
    SmtpAccount, default_path, load, load_str,
};
pub use contacts::connect_carddav_contact_providers;
pub use contacts_edit::{ContactEdit, build_contact_draft, build_contact_patch};
use engine_core::{ids::AccountId, sync::SyncUpdate};
use engine_provider::Provider;
pub use error::AccountError;
pub use event_detail::{DetailOccurrence, EventDetail, project_event_detail};
pub use google::{
    GoogleConfig, connect_google_calendar_providers, connect_google_contact_providers,
    connect_google_folder, connect_google_mail_providers, fetch_google_primary_address,
    google_token_source, load_google_str,
};
pub use graph::{
    CredentialOrigin, GraphTokenSource, TokenSink, connect_graph_calendar_providers,
    connect_graph_folder, connect_graph_mail_providers,
};
pub use imap::{
    ImapConnections, connect_imap_mailbox, connect_imap_watcher, connect_mail_providers,
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
    DEFAULT_VISIBLE_HOURS, DefaultCalendar, EffectiveSync, MAX_PUSH_FOLDERS, MAX_SENDER_NAME_CHARS,
    MAX_VISIBLE_HOURS, MESSAGE_SIZE_LIMITS_MB, MIN_VISIBLE_HOURS, MessageGrouping,
    MessageSizeLimit, POLL_INTERVALS, Preferences, QuoteStyle, ReplyFallback, SYNC_DEPTHS,
    SwipeAction, SyncDepth, SyncStrategy, TimeFormat, WeekStart, cap_push_folders,
    clamp_visible_hours, effective, load_preferences, preferences_path, sanitize_sender_name,
    save_preferences, snap_poll_interval,
};
use provider_caldav::{CalDavConfig, CalDavProvider, Credentials};
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
pub use setup::{AccountSetup, build_config_toml, imap_default_port, smtp_default_port};
pub use signatures::{
    AccountSignatureAssignment, SignatureId, SignatureSlot, Signatures, StoredSignature,
    load_signatures, save_signatures, signatures_path,
};

use crate::{setup::normalize_caldav_base_url, tls::account_tls};

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
    let tls = account_tls(account)?;
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
    // Ungated, for the reason `connect_carddav_contact_providers` gives: no DAV adapter
    // states a ceiling yet.
    .with_retry(throttle::ungated_retry())
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
