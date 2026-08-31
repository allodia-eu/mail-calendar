//! Consented product analytics: the consent gate, the install id, and the sink port.
//!
//! **Consent is the gate, and its absence is a refusal.** Nothing is minted, built, or sent
//! until the user opts in. Under ePrivacy Art. 5(3) the *act* of writing an identifier to the
//! device needs consent regardless of whether the data is personal: so the install id is minted
//! **at the moment of consent**, not at first launch. A user who never opts in has nothing on
//! disk for analytics at all, and withdrawal deletes the id and asks the backend to erase
//! everything held under it. The full contract, and the law behind it, is in `docs/analytics.md`.
//!
//! Delivery is somebody else's problem: this module builds a [`Batch`] and hands it to a
//! [`TelemetrySink`]. `mailcal-telemetry` implements that port over HTTPS; the demo, the
//! showcase, and every test run with no sink at all, so the core stays network-free and
//! offline-testable.

use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

use engine_api::{AccountId, Provider, UtcDateTime};
use mailcal_account::{load_preferences, save_preferences};

use crate::{App, Intent};

mod event;
mod payload;

pub use event::{DurationBucket, Event, Feature, Protocol};
use payload::AccountMix;
pub use payload::{
    Batch, Context, DeviceClass, DeviceInfo, PROPERTY_KEYS, Platform, SCHEMA, WireEvent,
};

/// The version of the consent notice the current payload corresponds to. Bump it when what we
/// send materially widens: a stored consent at an older version is **stale**, so the app asks
/// again rather than silently reading the old "yes" as covering the new data.
///
/// - v1; initial consented payload (`docs/analytics.md`).
/// - v2; added the `has_google` context flag (a Google/Gmail account in the mix), which widens the
///   batch context, so consent is re-asked. The relay's ingest whitelist must gain `has_google` in
///   lockstep.
pub const NOTICE_VERSION: u32 = 2;

/// Where consented events go. Implemented by `mailcal-telemetry` over HTTPS; absent in the demo,
/// the showcase, and every test.
///
/// Both methods are called from the runtime's worker threads and **must not block**; an
/// implementation queues and returns. A sink that cannot reach the network stays silent: a
/// self-hosted or air-gapped deployment must run identically with the endpoint unreachable
/// (sovereignty doctrine, enforcement principle 4).
pub trait TelemetrySink: Send + Sync + core::fmt::Debug {
    /// Queues one batch for delivery. Best-effort: dropping it is always allowed.
    fn send(&self, batch: Batch);

    /// Asks the backend to erase everything held under `install_id`: the user withdrew consent
    /// (GDPR Art. 17). Best-effort; the local id is cleared regardless.
    fn erase(&self, install_id: String);
}

/// What the host needs to decide whether to put the consent question up, and what to show in
/// Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalyticsConsent {
    /// Whether the consent question is **settled**. `false` on first run, that, and only that,
    /// is what puts the welcome screen up. Once asked we never ask again unless [`NOTICE_VERSION`]
    /// moves. It also reads `true` in a build with no preferences store (the demo, the showcase),
    /// where the question is settled by construction: nothing can be written or sent.
    pub asked: bool,
    /// Whether analytics is on. `false` unless the user actively opted in.
    pub enabled: bool,
}

/// The persisted consent decision and the install id it licenses.
pub(crate) struct TelemetryState {
    consent: Option<bool>,
    install_id: Option<String>,
    notice_version: Option<u32>,
    prefs_path: Option<PathBuf>,
}

impl TelemetryState {
    /// Loads the stored decision (unasked, with no install id, when absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        Self {
            consent: prefs.analytics_consent,
            install_id: prefs.analytics_install_id,
            notice_version: prefs.analytics_notice_version,
            prefs_path,
        }
    }

    /// The consent decision as the host sees it. A stored "yes" against an **older** notice
    /// version reads back as *not asked* and *not enabled*: so widening the payload re-asks
    /// instead of quietly inheriting a consent that was given for less.
    fn consent(&self) -> AnalyticsConsent {
        // No store: the demo, the showcase, and most tests. Nothing can be written to the device
        // and nothing can be sent, so there is nothing to consent to *and* nowhere to record an
        // answer: a consent screen here could never be dismissed for good, and would reappear on
        // every launch. Report the question as settled, and settled **off**.
        if self.prefs_path.is_none() {
            return AnalyticsConsent {
                asked: true,
                enabled: false,
            };
        }
        let current = self.notice_version == Some(NOTICE_VERSION);
        match self.consent {
            Some(true) if current => AnalyticsConsent {
                asked: true,
                enabled: true,
            },
            // Declined stays declined across a notice bump: we asked once and were told no.
            Some(false) => AnalyticsConsent {
                asked: true,
                enabled: false,
            },
            _ => AnalyticsConsent {
                asked: false,
                enabled: false,
            },
        }
    }

    /// The install id events may carry; `Some` **only** while consent is live and current.
    /// Every emit path goes through this, so the gate cannot be bypassed by forgetting a check.
    fn install_id(&self) -> Option<String> {
        self.consent()
            .enabled
            .then(|| self.install_id.clone())
            .flatten()
    }

    /// Records the user's decision. Opting in mints a fresh install id; opting out clears it and
    /// returns the id that was in force, so the caller can ask the backend to erase it.
    fn set(&mut self, enabled: bool) -> Option<String> {
        let withdrawn = (!enabled).then(|| self.install_id.take()).flatten();
        self.consent = Some(enabled);
        self.notice_version = Some(NOTICE_VERSION);
        if enabled {
            self.install_id = Some(mint_install_id());
            self.consented_at_now();
        } else {
            self.install_id = None;
        }
        self.persist();
        withdrawn
    }

    /// Stamps the moment of consent. GDPR Art. 7(1): a controller must be able to *demonstrate*
    /// that consent was given, which means recording when.
    fn consented_at_now(&mut self) {
        // Held only until `persist` writes it; there is no in-memory reader.
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.analytics_consented_at = Some(now_rfc3339());
            let _ = save_preferences(path, &prefs);
        }
    }

    /// Writes the decision back read-modify-write, so the sibling display-zone / sync-depth /
    /// quote-style / swipe preferences survive.
    fn persist(&self) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        let mut prefs = load_preferences(path);
        prefs.analytics_consent = self.consent;
        prefs.analytics_install_id.clone_from(&self.install_id);
        prefs.analytics_notice_version = self.notice_version;
        if self.consent != Some(true) {
            // Withdrawal erases the timestamp too; there is no live consent to demonstrate.
            prefs.analytics_consented_at = None;
        }
        let _ = save_preferences(path, &prefs);
    }
}

/// Everything the analytics feature needs, grouped so `App` grows one field rather than four.
///
/// [`Telemetry::off`] is the no-sink shape, nothing is ever built or sent. Whether it still puts
/// the consent screen up depends on the path it is given, and the two cases are deliberate:
/// `off(Some(path))` is a **real build with no relay baked in** (every local build), which asks and
/// records the answer so the screen can be exercised end to end; `off(None)` is the **demo and the
/// showcase**, which have no store at all, so the question is settled off and no screen appears;
/// otherwise it would reappear on every launch and no UI automation could get past it.
pub struct Telemetry {
    state: Mutex<TelemetryState>,
    sink: Option<Box<dyn TelemetrySink>>,
    device: Option<DeviceInfo>,
    /// Account id → protocol family. The binding layer is the only layer that knows which
    /// protocol a `P: Provider` actually speaks, so it pushes this map down whenever the
    /// account set changes ([`App::set_accounts`]). It carries **no addresses**: the id is a
    /// key we look a protocol up by and never send, only the derived [`AccountMix`] goes on
    /// the wire.
    accounts: Mutex<BTreeMap<String, Protocol>>,
}

impl core::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the install id: it is the one identifying value the feature holds.
        f.debug_struct("Telemetry")
            .field("has_sink", &self.sink.is_some())
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

impl Telemetry {
    /// The live shape: a host-reported device, a sink, and consent persisted at `prefs_path`.
    #[must_use]
    pub fn new(
        prefs_path: Option<PathBuf>,
        device: DeviceInfo,
        sink: Box<dyn TelemetrySink>,
    ) -> Self {
        Self {
            state: Mutex::new(TelemetryState::new(prefs_path)),
            sink: Some(sink),
            device: Some(device),
            accounts: Mutex::new(BTreeMap::new()),
        }
    }

    /// A real build that knows its device but has **no relay baked in**, which is every local
    /// build, and any release built without `ALLODIA_TELEMETRY_URL`.
    ///
    /// It still asks for consent and records the answer, and
    /// [`analytics_payload_preview`](App::analytics_payload_preview) still renders the **true**
    /// payload. That matters: the preview is a promise about what we would send, so a build that
    /// showed a hollow one (`os_version: "0"`, `device_class: "unknown"`) would be lying to the
    /// user on the very screen whose entire purpose is not to. It simply has nowhere to send it.
    #[must_use]
    pub fn unsent(prefs_path: Option<PathBuf>, device: DeviceInfo) -> Self {
        Self {
            state: Mutex::new(TelemetryState::new(prefs_path)),
            sink: None,
            device: Some(device),
            accounts: Mutex::new(BTreeMap::new()),
        }
    }

    /// Analytics disabled outright: no device, no sink, nothing ever sent. The demo, the
    /// showcase, and every test.
    #[must_use]
    pub fn off(prefs_path: Option<PathBuf>) -> Self {
        Self {
            state: Mutex::new(TelemetryState::new(prefs_path)),
            sink: None,
            device: None,
            accounts: Mutex::new(BTreeMap::new()),
        }
    }
}

impl<P: Provider> App<P> {
    /// Whether the user has been asked about analytics, and whether it is on. The host pulls
    /// this at boot to decide whether to show the consent screen, and to render the Settings
    /// toggle.
    #[must_use]
    pub fn analytics_consent(&self) -> AnalyticsConsent {
        self.telemetry
            .state
            .lock()
            .expect("telemetry mutex poisoned")
            .consent()
    }

    /// Records the user's analytics decision.
    ///
    /// Opting in mints the install id. Opting out clears it **and** asks the backend to erase
    /// everything held under it (GDPR Art. 17), which is possible precisely *because* the id is
    /// stable. Either way the decision is persisted, so we ask exactly once.
    pub fn set_analytics_consent(&self, enabled: bool) {
        let withdrawn = self
            .telemetry
            .state
            .lock()
            .expect("telemetry mutex poisoned")
            .set(enabled);
        if let (Some(id), Some(sink)) = (withdrawn, self.telemetry.sink.as_ref()) {
            sink.erase(id);
        }
        log::info!("analytics consent set: enabled={enabled}");
        if enabled {
            // The session in which consent was given is a real session: count it now, or it
            // stays invisible until the next launch (the boot-time report already no-op'd).
            self.report_app_opened();
        }
    }

    /// Records which protocol each configured account speaks. The binding layer calls this
    /// whenever the account set changes (boot, add, remove); it is the only layer that knows a
    /// provider's protocol family. The ids are keys we look protocols up by; they never leave
    /// the device.
    pub fn set_accounts(&self, accounts: BTreeMap<String, Protocol>) {
        *self
            .telemetry
            .accounts
            .lock()
            .expect("account-mix mutex poisoned") = accounts;
    }

    /// The protocol an account speaks, if the binding layer has told us about it. `None` in the
    /// demo/tests, where sync events are dropped anyway.
    fn protocol_of(&self, account_id: &str) -> Option<Protocol> {
        self.telemetry
            .accounts
            .lock()
            .expect("account-mix mutex poisoned")
            .get(account_id)
            .copied()
    }

    /// Reports one account's sync pass: the sync-health signal ("is JMAP flakier than IMAP?
    /// slower?"). `reachable` is the engine's own reachability verdict, already computed for the
    /// outage badge; `elapsed_ms` is that account's own pass, bucketed here.
    ///
    /// Nothing about the account crosses the wire: the id is only the key we look its protocol
    /// up by.
    pub(crate) fn track_sync(&self, account_id: &AccountId, reachable: bool, elapsed_ms: u64) {
        let Some(protocol) = self.protocol_of(account_id.as_str()) else {
            return;
        };
        self.track(if reachable {
            Event::SyncCompleted {
                protocol,
                duration: DurationBucket::of_millis(elapsed_ms),
            }
        } else {
            Event::SyncFailed { protocol }
        });
    }

    /// Reports a launch: the retention signal, plus a snapshot of the user's settings so we
    /// learn what the defaults *should* be. The host calls this once per launch, after boot.
    /// A no-op until the user opts in, and opting in emits it immediately, so the session in
    /// which consent was given is counted rather than being invisible until the next launch.
    pub fn report_app_opened(&self) {
        self.track(Event::AppOpened);
        // Read the persisted settings straight from the preferences file: these are exactly the
        // values whose drift from the defaults we want to learn about, and reading them here
        // needs no accessor on each settings state.
        let prefs = self
            .prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        self.track(Event::SettingsSnapshot {
            grouping: prefs.message_grouping,
            quote_style: prefs.quote_style,
            swipe_left: prefs.swipe_left,
            swipe_right: prefs.swipe_right,
        });
    }

    /// The exact JSON we would send right now, for the consent screen's "see exactly what we
    /// send" panel and the Settings screen. Built from the **same** [`Batch`] type the sink
    /// serializes, so the preview cannot drift from what actually goes on the wire.
    ///
    /// Rendered with a placeholder id before consent; there is no real one yet, because we do
    /// not mint one until asked.
    #[must_use]
    pub fn analytics_payload_preview(&self) -> String {
        let install_id = self
            .telemetry
            .state
            .lock()
            .expect("telemetry mutex poisoned")
            .install_id()
            .unwrap_or_else(|| "<generated when you opt in>".to_owned());
        let device = self.telemetry.device.clone().unwrap_or(DeviceInfo {
            platform: Platform::Macos,
            os_version: "0".to_owned(),
            device_class: DeviceClass::Unknown,
            app_version: "0.0.0".to_owned(),
            locale: "en".to_owned(),
        });
        let context = Context::build(&device, self.account_mix());
        let batch = Batch::new(install_id, context, &[Event::AppOpened]);
        serde_json::to_string_pretty(&batch).unwrap_or_else(|_| "{}".to_owned())
    }

    /// The current account mix, folded from the protocol map: a bucketed count plus one bool
    /// per protocol family. The account ids the map is keyed by are **not** part of it.
    fn account_mix(&self) -> AccountMix {
        AccountMix::of(
            self.telemetry
                .accounts
                .lock()
                .expect("account-mix mutex poisoned")
                .values()
                .copied(),
        )
    }

    /// Classifies an inbound [`Intent`] as a product surface, for feature-adoption counts.
    ///
    /// This is a match on the intent's **variant**. The only field it ever reads is whether a
    /// search query is blank and whether an attachment list is empty; never a query, a subject,
    /// a recipient, a body, an event title, or a filename. Feature adoption therefore cannot
    /// become a content leak by accident, and the whole rule is auditable in one function.
    /// Intents that are navigation, refreshes, or settings writes are not features and emit
    /// nothing.
    pub(crate) fn track_intent(&self, intent: &Intent) {
        let (feature, attachments) = match intent {
            Intent::Search(Some(query)) if !query.trim().is_empty() => (Feature::Search, None),
            Intent::RefreshCalendar => (Feature::Calendar, None),
            Intent::CreateEvent { .. } => (Feature::EventCreate, None),
            Intent::SubmitMail { .. } => (Feature::ComposerNew, None),
            Intent::SubmitRichMail { blobs, .. } => (Feature::ComposerNew, Some(blobs)),
            Intent::SubmitRichReply { blobs, .. } => (Feature::ComposerReply, Some(blobs)),
            Intent::SubmitRichForward { blobs, .. } => (Feature::ComposerForward, Some(blobs)),
            _ => return,
        };
        self.track(Event::FeatureUsed { feature });
        if attachments.is_some_and(|blobs| !blobs.is_empty()) {
            self.track(Event::FeatureUsed {
                feature: Feature::AttachmentAdd,
            });
        }
    }

    /// Emits one event; **the single emit path**, and the only place the consent gate lives.
    ///
    /// No consent, no current notice version, no device, or no sink → nothing is built and
    /// nothing is sent. Cheap enough to call unconditionally from a hot path: with analytics off
    /// it is a lock, a `None`, and a return.
    ///
    /// Public because the binding layer owns account setup (it is the only layer that sees a
    /// connection attempt fail), so it emits the funnel events. That is safe to expose: [`Event`]
    /// is a closed enum whose every field is another closed enum, so a caller cannot widen what
    /// we send: only add a variant here, which also forces a key into [`PROPERTY_KEYS`] and the
    /// relay's whitelist.
    pub fn track(&self, event: Event) {
        let Some(sink) = self.telemetry.sink.as_ref() else {
            return;
        };
        let Some(device) = self.telemetry.device.as_ref() else {
            return;
        };
        let Some(install_id) = self
            .telemetry
            .state
            .lock()
            .expect("telemetry mutex poisoned")
            .install_id()
        else {
            return;
        };
        let context = Context::build(device, self.account_mix());
        sink.send(Batch::new(install_id, context, &[event]));
    }
}

/// A fresh, opaque install id: 16 bytes of CSPRNG output, base64url. Not derived from the
/// device, the accounts, the addresses, or anything else; it identifies nothing but itself, and
/// it exists only because the user said yes. (Same construction as the OAuth CSRF `state`.)
fn mint_install_id() -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::{SecureRandom, SystemRandom};

    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system CSPRNG fills 16 bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The current instant as RFC3339 (`…Z`), matching how the notification high-water-marks are
/// stored in the same preferences file.
fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    UtcDateTime::new(
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
    .expect("a civil UTC time from the system clock is always representable")
    .to_string()
}
