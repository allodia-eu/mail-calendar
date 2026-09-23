//! UniFFI Kotlin/Swift bindings over `mailcal-app`.
//!
//! The thin FFI surface the native macOS/Android clients consume: a [`MailcalApp`]
//! object exposing the unidirectional loop; `dispatch` an [`Intent`] (fire-and-forget,
//! scheduled on an internal runtime so native never awaits Rust), and the app notifies
//! a foreign [`Observer`] when a [`Surface`] changes, which the host pulls a snapshot
//! for ([`MailcalApp::mailbox_list`]). FFI types mirror the pure `mailcal-app` types so
//! the app stays binding-free.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use engine_api::{AccountId, Provider};
use mailcal_app::{Account, App};
use tokio::runtime::Runtime;

mod about;
mod account_registry;
mod account_repair;
mod agent_ui;
mod allodia;
mod allodia_health;
#[cfg(feature = "allodia-license")]
mod allodia_pass;
mod allodia_purchase;
#[cfg(feature = "allodia-license")]
mod allodia_style_pass;
mod allodia_subscription;
mod allodia_sync;
#[cfg(feature = "allodia-license")]
mod allodia_tokens;
#[cfg(feature = "allodia-license")]
mod allodia_transport;
// The person's own AI endpoint: its key in the secure store, and which backend AI goes through.
mod ai_endpoint;
#[cfg(feature = "allodia-license")]
mod ai_relay;
mod ai_transport;
mod analytics;
mod app_accounts;
mod app_accounts_google;
mod app_accounts_microsoft;
mod app_allodia;
mod app_allodia_purchase;
mod app_allodia_subscription;
mod app_allodia_sync;
mod app_calendar;
mod app_contacts;
mod app_display;
mod app_month;
mod app_sender_name;
mod app_settings;
mod app_signatures;
mod app_snapshots;
mod app_writing_styles;
mod autodetect;
mod background;
mod background_sync;
mod blocking;
mod boot;
mod composer;
mod composer_files;
mod composer_reply;
mod connected_account;
mod connection_log;
mod connector;
mod convert;
mod convert_mailbox;
mod convert_reading;
mod convert_settings;
mod convert_writing_style;
mod crash;
mod credential_log;
pub mod credential_store;
mod demo;
mod drafts;
mod error;
mod google;
mod jmap_oauth;
mod logging;
mod mailto;
mod mcp;
mod microsoft;
mod native_fault;
mod native_fault_record;
#[cfg(windows)]
mod native_fault_windows;
mod oauth_routes;
mod observer;
mod protocol;
mod protocol_surface;
mod reconnect;
mod records;
mod records_avatar;
mod records_calendar;
mod records_connectivity;
mod records_contacts;
mod records_folders;
mod records_outbox;
pub mod sync_state;
// The meeting-invitation card: its own file, since `records.rs` is at the 500-line limit.
mod records_invitation;
mod records_recurrence;
mod records_repeat_summary;
mod records_writing_style;
mod rendering;
mod repeat_editor;
mod runtime;
pub mod save;
mod setup;
mod share;
mod showcase;
mod showcase_bodies;
mod showcase_contacts;
mod showcase_data;
mod timezone;
mod token_sink;

pub use about::{AboutInfo, AboutPlatform, Attribution, about_info};
pub use agent_ui::{AgentDraft, AgentHostUi};
pub use ai_endpoint::{OwnAiEndpoint, OwnEndpointError, is_reserved_config};
pub use allodia::{AllodiaAccount, AllodiaSignInStart, allodia_sign_in_available};
pub use allodia_health::AllodiaGrantHealth;
pub use allodia_purchase::{
    AllodiaOffer, AllodiaPlan, AllodiaPurchaseError, AllodiaPurchaseReport, AllodiaStore,
    AllodiaStoreProduct, AllodiaStorePurchase,
};
pub use allodia_subscription::{
    AllodiaBiller, AllodiaCancellation, AllodiaCheckout, AllodiaIntervalChange, AllodiaOwnStatus,
    AllodiaOwnSubscription, AllodiaPrices, AllodiaStoreStatus, AllodiaStoreSubscription,
    AllodiaSubscription, AllodiaSubscriptionActions, AllodiaSubscriptionRefusal,
};
pub use allodia_sync::{
    AllodiaAccountChange, AllodiaAccountKind, AllodiaAccountOffer, AllodiaAccountSyncMode,
    AllodiaStyleConflict, AllodiaSyncReport, setup_from_offer,
};
pub use analytics::{AnalyticsConsent, DeviceClass, DeviceInfo, Platform};
pub use app_display::stored_appearance;
pub use app_month::calendar_palette;
pub use app_sender_name::sender_label;
pub use autodetect::{
    DetectedServerRow, DnsError, MissReason, MxRecord, MxResolution, MxResolver,
    SetupRecommendation, SrvRecord, SrvResolution,
};
use background::BackgroundManager;
pub use background_sync::{AccountNewMail, BackgroundSyncOutcome, NewMailPreview};
pub use composer::{
    ComposerBlob, Recipients, forward_subject, render_composer_document_json, reply_subject,
};
pub use composer_files::{
    ComposerFileAttachment, MAX_INLINE_IMAGE_BYTES, composer_image_data_url,
    composer_image_data_url_from_bytes,
};
pub(crate) use connected_account::ConnectedAccount;
pub use credential_store::{AccountCredentialStore, CredentialStoreError};
pub use error::MailcalError;
pub use google::{GoogleLoginStart, begin_google_login};
pub use logging::{LogLevel, Logger};
pub use mailto::{MailtoPrefill, parse_mailto_uri};
pub use microsoft::{MicrosoftLoginStart, begin_microsoft_login};
pub use native_fault::watch_for_native_faults;
pub use oauth_routes::{OAuthRoutes, oauth_routes};
pub use protocol::{
    BulkAction, Intent, InvitationResponse, OutboxIntent, SearchScope, SelectedRow,
};
pub use protocol_surface::{Observer, Surface};
pub use records::{
    AccountRow, AccountSyncProgress, AttachmentRow, CalendarWriteStatus, EmptyReason, FlatRow,
    MailboxListSnapshot, ReadingSnapshot, RecipientSuggestion, SearchHorizon, SendStatus,
    SnapshotRow, SyncProgressSnapshot, ThreadMessage, ThreadRow, TimeZoneSnapshot, UnfiledCopy,
    ViewMode,
    settings::{
        AccountSignatureRow, AccountSyncRow, DefaultMailAppOutcome, DefaultMailAppSupport,
        McpAccountRow, McpSettings, QuoteSettings, QuoteStyleKind, SignatureBody, SignatureRow,
        SignatureSlotKind, SignaturesSnapshot, SwipeActionKind, SwipeDirection, SwipeSettings,
        SyncFolderRow, SyncSettingsSnapshot, SyncStrategyKind,
    },
};
pub use records_avatar::Avatar;
pub use records_calendar::{
    AllDayBand, Appearance, CalendarColor, CalendarLayout, CalendarPage, CalendarRow,
    CalendarSnapshot, DisplaySettings, EventAttendee, EventDetail, EventEdge, EventRow, GridDay,
    MonthCell, MonthChip, MonthPage, Swatch, TimeFormat, TimedSegment, WeekStart,
};
pub use records_connectivity::{
    AccountProvider, ConnectionInfo, ConnectivitySnapshot, HttpVersion, TlsVersion,
};
pub use records_contacts::{
    ContactCardRef, ContactDetail, ContactEdit, ContactRow, ContactTarget, ContactValue,
    ContactWriteStatus, ContactsSnapshot, RecipientMatch,
};
pub use records_folders::{AccountFolderRow, FolderRole, FolderRow};
pub use records_invitation::{
    AttendeeTally, InvitationCard, InvitationKind, InvitationPreview, ReplyPrompt, ResponseStatus,
};
pub use records_outbox::{ComposeRequest, QueuedRow, QueuedState};
pub use records_recurrence::{
    EventRecurrence, ProposedEdit, RecurrenceChange, RecurrenceDay, RecurrenceEnd,
    RecurrenceFrequency, RecurrenceWeekday, RepeatDraft, SeriesEditWarning, SimpleRecurrence,
};
pub use records_repeat_summary::{RepeatRhythm, RepeatStop, RepeatSummary};
pub use records_writing_style::{
    AccountWritingStyleRow, AiCharge, AiRoute, CorpusLanguage, CorpusReport, CreditBalance,
    DraftReply, GateRefusal, HabitRow, JurisdictionClass, JurisdictionMode, LanguageStyleRow,
    LearnReport, LearningProgress, LearningStage, WritingStyleDetail, WritingStyleFailure,
    WritingStyleRow, WritingStyleSnapshot,
};
pub use rendering::{
    MessageCanvas, message_canvas, render_message_html, should_open_external_link,
};
pub use repeat_editor::repeat_change_of;
pub use setup::{
    AccountSetup, ConnectionSecurity, JmapSetup, MailServerKind, RejectedCertificate,
    account_config_toml, jmap_account_config_toml, standard_port,
};
pub use share::{
    RejectedShare, SharePrefill, ShareRejectionReason, ShareRequest, SharedFile, prefill_from_share,
};
pub use showcase_data::{
    ShowcaseInvitation, ShowcaseLocale, ShowcaseReply, showcase_invitation,
    showcase_locale_for_language, showcase_now, showcase_reply,
};
pub use sync_state::{SyncStateError, SyncStateStore};
pub(crate) use timezone::device_zone;
pub use timezone::{available_time_zones, device_time_zone};
/// A shared registry of each connected account's re-connection state; see
/// [`account_registry`], which explains why this is a type with three methods rather than the
/// open `HashMap` it replaced.
type SharedRegistry = Arc<account_registry::AccountRegistry>;

/// One account the app drives, with its providers boxed behind the [`Provider`] trait so
/// every account shares one concrete app type.
type BoxedAccount = Account<Box<dyn Provider>>;

uniffi::setup_scaffolding!();

/// The FFI entry point: owns the app and an internal runtime. `dispatch` schedules
/// work and returns at once (native never awaits Rust); the app later notifies the
/// observer, and the host pulls the snapshot.
#[derive(uniffi::Object)]
pub struct MailcalApp {
    runtime: Runtime,
    app: Arc<App<Box<dyn Provider>>>,
    /// Non-fatal account-connect diagnostics: at launch, any stored account whose IMAP/mail
    /// connect failed (a stale password, a server blip) and was skipped so the others come
    /// up. Account-prefixed. Empty when every stored account connected. Distinct from
    /// [`MailcalApp::calendar_connect_errors`] so a skipped *account* is never mislabeled a
    /// calendar failure.
    account_connect_errors: Mutex<Vec<String>>,
    /// Non-fatal calendar (CalDAV) connect diagnostics: an account whose mail is up but whose
    /// configured calendar provider couldn't connect (so its calendar is empty rather than
    /// missing by choice). Account-prefixed. Runtime additions (`add_account`) append here, so
    /// it is behind a `Mutex`. Empty when every configured calendar connected.
    calendar_connect_errors: Mutex<Vec<String>>,
    /// Each connected account's config, keyed by account id; shared with the on-demand
    /// [`HostConnector`] and re-read when a sync-depth change reconnects an account's providers.
    registry: SharedRegistry,
    /// The per-account background sync runtime: standing IMAP `IDLE` watches (push) and
    /// poll timers, (re)started from the core's sync-settings snapshot whenever an account
    /// is added or its sync behaviour changes. Behind an [`Arc`] so a reconnect task can restart
    /// a recovered account's watches/poll off the runtime (see [`MailcalApp::retry_connections`]).
    background: Arc<BackgroundManager>,
    /// The local MCP (AI assistant access) server. Off until the user turns it on **and** a host
    /// has set an endpoint; `apply` is abort-then-respawn, exactly like [`BackgroundManager`], so
    /// a settings change takes effect without a restart. A host that never calls
    /// [`MailcalApp::set_mcp_endpoint`] can never listen, which is how iOS and Android are
    /// excluded, by construction rather than by a check.
    mcp: Arc<mailcal_mcp::McpServer>,
    /// Where the MCP server listens, as the host set it. `None` on a platform that set none.
    /// Behind a `Mutex` because it arrives after construction, like the credential stores.
    mcp_endpoint: Mutex<Option<String>>,
    /// The host's composer port, so an assistant's `create_draft` opens a prefilled, unsent
    /// draft in the app's own composer. Empty until a client installs one; a client that never
    /// does simply reports that it has no composer.
    agent_ui: agent_ui::AgentUiSlot,
    /// The access token the account service is called with, and what is needed to mint one. Empty
    /// until something asks. A build with no Allodia registration has no grant to refresh and no
    /// service to refresh it against, so it carries no field either.
    #[cfg(feature = "allodia-license")]
    allodia_tokens: allodia_tokens::Tokens,
    /// What this device has learned about its Allodia sign-in; see [`AllodiaGrantHealth`].
    ///
    /// Recorded only from evidence: a refusal the service actually gave, or a scope set it
    /// actually issued. A pass that could not reach anything leaves it alone, which is what stops
    /// an outage from signing somebody out. Reset to `Ok` by a sign-in and by any call that
    /// succeeds.
    #[cfg(feature = "allodia-license")]
    allodia_health: Mutex<AllodiaGrantHealth>,
    /// Store purchases this process has not yet had granted; see [`allodia_license::Ledger`].
    /// Nothing in it is written down and nothing needs to be: the store is the durable copy.
    #[cfg(feature = "allodia-license")]
    allodia_purchases: Mutex<allodia_license::Ledger>,
    /// Where this device remembers what it has synced with the account service, once a host has
    /// installed somewhere to keep it. `None` until then, which is a wiring bug rather than a
    /// state a pass may quietly run in; see [`crate::app_allodia_sync`].
    allodia_sync: Mutex<Option<Arc<sync_state::SyncBookkeeping>>>,
    /// The signed-in **Allodia account**, restored from the host's store at boot and replaced by a
    /// sign-in. `None` when nobody is signed in, which is every build from source and every
    /// install that has not asked.
    ///
    /// Not an account in the mail sense: it has no mailbox, is in no switcher and syncs nothing.
    /// What it holds is the grant that lets the app ask Allodia's own service what this person is
    /// entitled to: so it sits beside the account list rather than in it. See [`crate::allodia`].
    allodia: Mutex<Option<allodia::StoredAccount>>,
    /// The own AI endpoint's key, restored from the host's store at boot ([`crate::ai_endpoint`]).
    ai_key: Mutex<Option<String>>,
    /// This app, weakly: what a callback the core holds (the relay's token source) reaches back
    /// through without keeping the app alive.
    #[cfg_attr(
        not(feature = "allodia-license"),
        expect(dead_code, reason = "only the relay reaches back")
    )]
    this: std::sync::Weak<MailcalApp>,
    /// The host's OS-secure-store writer, supplied **at construction** and shared with the token
    /// sink so a rotated refresh token is re-persisted. One store serves all three OAuth
    /// families; it can never be absent, which is the point; see
    /// [`credential_store`](crate::credential_store) for what an absent one cost.
    credential_store: Arc<dyn credential_store::AccountCredentialStore>,
    /// Ids of accounts that have **no live providers** this session: a boot connect that failed
    /// (kept as a placeholder so it still lists, badged unreachable), or a reconnect that hasn't
    /// yet succeeded. A Refresh or return-to-online drains this and re-runs the full connect for
    /// each ([`MailcalApp::retry_connections`]); a successful reconnect removes the id.
    disconnected: Arc<Mutex<HashSet<String>>>,
    /// The device's display zone (resolved at construction), used as the `Prefer:
    /// outlook.timezone` a reconnecting Microsoft account rebinds its Graph calendar provider
    /// with: so a recurring series expands DST-correctly. Fixed for the app's lifetime; the
    /// user's *chosen* display zone re-projects the agenda in the view-model, this only anchors
    /// the Graph read (one display zone per provider, per the engine's documented limitation).
    device_zone: engine_api::TimeZoneId,
    /// Whether this app was built by [`MailcalApp::new_showcase`]: the seeded, offline
    /// screenshot dataset.
    ///
    /// Read by exactly one thing: [`MailcalApp::detect_account_settings`], which is the only
    /// FFI call that would otherwise reach the network from a showcase build. Detection *is* a
    /// network operation, so without this the account-setup documentation could never be
    /// screenshotted: a capture would show a spinner, or whatever the developer's own DNS
    /// happened to answer, which is neither deterministic nor safe to publish.
    ///
    /// A flag rather than a reserved domain (`*.example` can never resolve, so it was tempting):
    /// a build that has not opted into the showcase should behave identically for every address,
    /// and a surprising special case in shipped code is worth more than the field it saves.
    showcase: bool,
}

impl MailcalApp {
    /// Re-runs the full connect for every account sitting **disconnected** (a boot outage kept
    /// as a placeholder, or a prior retry that failed) and joins each that succeeds back into the
    /// app with live providers: so a recovered provider heals without an app restart, regaining
    /// its role folders, capabilities, and calendar (not a degraded INBOX-only state). The
    /// disconnected set is drained optimistically so a concurrent trigger (a Refresh racing a
    /// return-to-online) never dials an account twice; a still-failing account is re-queued for
    /// the next attempt. Fire-and-forget on the runtime, so a slow re-dial never blocks the
    /// host's `dispatch`. A no-op when nothing is disconnected.
    fn retry_connections(&self) {
        let ids: Vec<String> = {
            let mut disconnected = self
                .disconnected
                .lock()
                .expect("disconnected mutex poisoned");
            if disconnected.is_empty() {
                return;
            }
            disconnected.drain().collect()
        };
        // Snapshot each account's reconnection plan under the registry lock (so no lock is held
        // across the re-dial); an id whose registry entry is gone (removed meanwhile) drops out.
        // The catch-up sync after a successful reconnect resolves the account's own depth.
        let plans: Vec<(AccountId, account_registry::AccountDial)> = ids
            .into_iter()
            .filter_map(|id| {
                let account_id = AccountId::try_from(id.as_str()).ok()?;
                Some((account_id, self.registry.dial(&id)?))
            })
            .collect();
        if plans.is_empty() {
            return;
        }
        log::info!(
            "retry_connections: re-dialing {} disconnected account(s)",
            plans.len(),
        );
        self.runtime.spawn(reconnect::reconnect_all(
            Arc::clone(&self.app),
            Arc::clone(&self.background),
            Arc::clone(&self.registry),
            Arc::clone(&self.disconnected),
            plans,
            self.device_zone.clone(),
        ));
    }

    /// Recomputes and restarts one account's background sync from the current settings;
    /// called after an account is added or its sync behaviour changes.
    fn refresh_background(&self, account_id: &str) {
        let snapshot = self.runtime.block_on(self.app.sync_settings());
        let row = snapshot
            .accounts
            .iter()
            .find(|row| row.account_id == account_id);
        self.background.apply(account_id, row);
    }
}

/// Joins one diagnostic channel's account-prefixed messages into a single newline-separated
/// string for a host to surface, or `None` when the channel is empty.
fn joined(errors: &Mutex<Vec<String>>) -> Option<String> {
    let errors = errors.lock().expect("diagnostics mutex poisoned");
    (!errors.is_empty()).then(|| errors.join("\n"))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_showcase;

#[cfg(test)]
mod tests_showcase_first_run;

#[cfg(test)]
mod tests_showcase_invitation;

#[cfg(test)]
mod tests_showcase_sender_name;

#[cfg(test)]
mod tests_boot;

#[cfg(test)]
mod tests_credentials;

#[cfg(test)]
mod tests_credential_ordering;

#[cfg(test)]
mod tests_calendar;

#[cfg(test)]
mod tests_setup;

#[cfg(test)]
mod tests_ai_endpoint;

#[cfg(all(test, feature = "allodia-license"))]
mod tests_ai_relay;
