//! App construction + account connection, split out of `lib.rs` to keep it under the
//! 500-line limit. The thin `#[uniffi::constructor]` wrappers on [`MailcalApp`] live in
//! `lib.rs` (so the generated FFI surface stays there) and delegate to the builders here;
//! no FFI macros live in this module.

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use engine_api::{AccountId, Engine};
use mailcal_account::{CredentialOrigin, TokenSink, preferences_path};
use mailcal_app::{App, TimeZoneInit};

use crate::{
    BoxedAccount, DeviceInfo, LogLevel, Logger, MailcalApp, MailcalError, Observer, SharedRegistry,
    account_registry::{AccountRegistry, dial_all},
    analytics,
    background::BackgroundManager,
    build_runtime, connection_log,
    connector::HostConnector,
    credential_store::AccountCredentialStore,
    device_zone, logging,
    observer::{DebouncedObserver, ObserverBridge},
    token_sink::token_sink,
};

mod connect;
mod contacts;
mod inmemory;
mod stored;

/// The host's device facts, grouped so the boot signature stays inside clippy's argument limit;
/// and because they belong together: both are "what the OS says about the machine we are on".
pub(crate) struct HostDevice {
    /// The device's current OS timezone (an IANA id).
    pub(crate) timezone: String,
    /// The raw device facts for analytics. The core coarsens them, nothing is sent without
    /// consent.
    pub(crate) info: DeviceInfo,
}

// Re-exported for the single-account `add_account` path (crate::MailcalApp) and its JMAP
// detection, plus the FFI-surface tests.
pub(crate) use connect::{connect_google_calendars, connect_jmap_calendars};
pub(crate) use contacts::{connect_caldav_contacts, connect_jmap_contacts};
// The in-memory demo/showcase builders (no real account, no network) live in their own module.
pub(crate) use inmemory::{build_demo, build_showcase};
pub(crate) use stored::{PreparedAccount, connect_graph_calendars, prepare_stored_account};

/// Builds a real account-backed app from the host's stored account `configs`: the body of
/// [`MailcalApp::new_accounts`]; see that method for the full contract.
///
/// # Errors
///
/// Returns [`MailcalError`] only for failures that prevent *any* account from working: the
/// runtime cannot start or the engine cannot open. A single account's connect failure is
/// non-fatal (recorded, not returned).
/// Which kind of boot this is: the one thing an interactive app and a headless background
/// worker decide differently: whether to run a live sync runtime (standing IMAP IDLE watches /
/// poll timers) or a single bounded pass and then quiesce.
pub(crate) struct BootMode {
    /// Whether to start the standing IMAP IDLE watches / poll timers.
    pub(crate) start_live_sync: bool,
}

/// The foreign objects the host supplies at construction, grouped because they are one thing
/// seen three ways: where the core announces a change, where its diagnostics go, and where a
/// credential it just updated must be durably written.
///
/// The store belongs here (beside the observer and the logger) rather than behind a setter,
/// because [`build_accounts`] starts dialing before it returns and a refresh can rotate a token
/// within milliseconds. See [`crate::credential_store`].
pub(crate) struct HostPorts {
    pub(crate) observer: Box<dyn Observer>,
    pub(crate) logger: Box<dyn Logger>,
    pub(crate) credential_store: Arc<dyn AccountCredentialStore>,
}

pub(crate) fn build_accounts(
    ports: HostPorts,
    log_level: LogLevel,
    mut configs: Vec<String>,
    data_dir: String,
    device: HostDevice,
    mode: BootMode,
) -> Result<Arc<MailcalApp>, MailcalError> {
    let BootMode { start_live_sync } = mode;
    let HostPorts {
        observer,
        logger,
        credential_store,
    } = ports;
    let HostDevice {
        timezone: device_timezone,
        info: device_info,
    } = device;
    // Resolve the device's display zone once: it seeds the app's active zone, is the `Prefer:
    // outlook.timezone` a Graph calendar provider reads with, and is carried on `MailcalApp` so a
    // later reconnect rebinds Graph calendars with the same zone.
    let device_tz = device_zone(device_timezone);
    // Install the host logger first, so every line below (connect timings, the engine
    // open) is captured from the start of boot.
    logging::install_logger(logger, log_level);
    let boot_start = Instant::now();
    let runtime = build_runtime().map_err(|err| MailcalError::Engine(err.to_string()))?;
    let base = PathBuf::from(&data_dir);
    let _ = std::fs::create_dir_all(&base);
    let prefs_path = preferences_path(&base);

    // One entry in the host's store is not a mail account: the Allodia grant. Take it out before
    // anything below tries to read it as a mailbox: a build with no Allodia sign-in does this too,
    // and simply drops what it takes, because the alternative is reporting an intact grant as a
    // corrupt account at every launch.
    let allodia = crate::allodia::take_stored(&mut configs);

    let registry = AccountRegistry::new();
    // The token sink every OAuth account's refresh shares, built over the host's store before
    // anything connects: so a rotation handed back by the very first refresh of this process
    // is persisted rather than dropped. Neither kind of boot has a later moment to install it:
    // the interactive one starts dialing inside this function, and a headless worker is gone
    // when its pass ends.
    let sink = token_sink(&registry, &credential_store);

    // Both modes prepare and **register** every stored account first, with no network at all: the
    // config parsed, the id derived, the account's one token source built, the entry written. That
    // is the ordering the token sink depends on (`crate::account_registry`), and it is now the same
    // ordering in both, because it is the same code.
    //
    // What differs is only *when the dial happens*, which is what `BootMode` is for. The
    // interactive app returns with provider-less placeholders and dials in the background, so it
    // can paint cached mail the instant it boots, that was the cure for the multi-second
    // "Connecting to your account…" wait, which used to sit on the critical path before first
    // paint. A headless worker dials here and now, because its one bounded
    // `run_background_sync` pass needs live providers before it runs and the process is gone
    // when the pass ends.
    let Prepared {
        accounts: placeholders,
        mut account_errors,
    } = prepare_accounts(&registry, &sink, &configs);

    let mut calendar_errors: Vec<String> = Vec::new();
    let mut failed: Vec<FailedDial> = Vec::new();
    let accounts: Vec<BoxedAccount> = if start_live_sync {
        // Nothing is dialed yet, so nothing is known-failed: each account shows its cached mail as
        // if connected until a background dial actually fails.
        placeholders
    } else {
        let connect_start = Instant::now();
        let dialed = runtime.block_on(dial_registered(&registry, placeholders, &device_tz));
        // A dial that failed keeps its placeholder: the account still lists, badged unreachable,
        // and re-dials later: so the account count is the same either way and only the channels
        // differ. Nothing is registered here: `prepare_accounts` did that before any of these
        // dialed, which is the property `crate::account_registry` exists to make unmissable.
        let mut connected = Vec::with_capacity(dialed.len());
        for (account, failure) in dialed {
            let id = account.id.as_str().to_owned();
            record_dial_outcome(
                account,
                id,
                failure,
                &mut connected,
                &mut account_errors,
                &mut calendar_errors,
                &mut failed,
            );
        }
        log::info!(
            "boot: {} reachable + {} disconnected of {} stored account(s) in {}ms",
            connected.len().saturating_sub(failed.len()),
            failed.len(),
            configs.len(),
            connect_start.elapsed().as_millis(),
        );
        for (index, account) in connected.iter().enumerate() {
            connection_log::log_account_connection_info(
                &format!("account[{index}]"),
                registry.account_type(account.id.as_str()),
                account,
            );
        }
        connected
    };
    let disconnected_ids: Vec<String> = if start_live_sync {
        accounts
            .iter()
            .map(|account| account.id.as_str().to_owned())
            .collect()
    } else {
        failed.iter().map(|failure| failure.id.clone()).collect()
    };

    let engine_start = Instant::now();
    let engine = Engine::open(base.join("mailcal.sqlite"))
        .map_err(|err| MailcalError::Engine(err.to_string()))?;
    log::info!(
        "boot: engine open+migrate in {}ms",
        engine_start.elapsed().as_millis(),
    );
    let lease_recovery_start = Instant::now();
    let abandoned = runtime
        .block_on(engine.abandon_sync_leases())
        .map_err(|err| MailcalError::Engine(err.to_string()))?;
    log::info!(
        "boot: abandoned {abandoned} interrupted sync scope lease(s) in {}ms",
        lease_recovery_start.elapsed().as_millis(),
    );
    let connector = HostConnector {
        registry: Arc::clone(&registry),
    };
    // Analytics. Consent is recorded in the same preferences file; the sink spawns its delivery
    // worker, so build it inside the runtime. A build with no relay baked in (every local build;
    // see `analytics::relay_config`) gets a telemetry that records consent and sends nothing.
    let telemetry = {
        let _guard = runtime.enter();
        analytics::build_telemetry(prefs_path.clone(), device_info)
    };
    let app = App::new(
        engine,
        accounts,
        TimeZoneInit {
            device_zone: device_tz.clone(),
            prefs_path: Some(prefs_path),
        },
        Some(Box::new(connector)),
        std::sync::Arc::new(DebouncedObserver::new(ObserverBridge { foreign: observer })),
        telemetry,
    );
    // Render the persisted store's mail before returning, so the host paints cached rows
    // the instant it comes up instead of a blank list while the background sync runs. The
    // host then dispatches RefreshMail, which re-snapshots and re-renders in the background.
    let app = Arc::new(app);
    // Seed what every account's synchronous headless dial already found, so the state is there the
    // instant the host pulls connectivity: the outage badge and its technical detail, or: for a
    // credential the server *refused*: the reconnect prompt instead, on the same terms
    // `reconnect_all` raises it (`docs/provider-oauth.md` rule 12). The deferred interactive path
    // has dialed nothing yet, so `failed` is empty here and both come from `reconnect_all`.
    for failure in &failed {
        let Ok(account_id) = AccountId::try_from(failure.id.as_str()) else {
            continue;
        };
        if failure.signin_rejected {
            app.note_signin_expired(&account_id);
        } else {
            app.note_account_unreachable(&account_id, Some(failure.detail.clone()));
        }
    }
    let disconnected: Arc<Mutex<HashSet<String>>> =
        Arc::new(Mutex::new(disconnected_ids.into_iter().collect()));
    let prime_start = Instant::now();
    runtime.block_on(app.prime_snapshot());
    log::info!(
        "boot: primed cached snapshot in {}ms; NewAccounts total {}ms",
        prime_start.elapsed().as_millis(),
        boot_start.elapsed().as_millis(),
    );
    // The calendar primes from the store too, but **off** the boot path, because the mail list
    // must not wait for it. It is a few hundred milliseconds of SQLite over a real calendar,
    // and blocking boot on it delayed the app's *primary* surface to pay for one the user has
    // not opened yet.
    //
    // Spawned, it is warm long before the Calendar tab is tapped, and it signals
    // `Surface::Calendar` when it lands so a host already sitting there re-pulls. Tap it inside
    // that window and the grid honestly says "loading this period…", which is what an unprimed
    // cache means, and what it said before any of this existed.
    //
    // Only the *painting* half is here. The sync that fills the store is in `reconnect_all`,
    // because at this point an interactive boot has provider-less placeholders and there is
    // nothing to sync with (`docs/calendar.md` §5).
    runtime.spawn({
        let app = Arc::clone(&app);
        async move { app.prime_calendar().await }
    });
    let background = Arc::new(BackgroundManager::new(
        Arc::clone(&app),
        Arc::clone(&registry),
        runtime.handle().clone(),
    ));
    // The MCP server is built beside the background manager and on the same runtime, but starts
    // STOPPED: it has no endpoint until a desktop host calls `set_mcp_endpoint`, and no accounts
    // until the user ticks one. A mobile host never calls either, so it never listens.
    let agent_ui: crate::agent_ui::AgentUiSlot = Arc::new(Mutex::new(None));
    let mcp = Arc::new(mailcal_mcp::McpServer::new(
        Arc::new(crate::mcp::AppBackend::new(
            Arc::clone(&app),
            Arc::clone(&agent_ui),
        )),
        runtime.handle().clone(),
    ));
    let mailcal = Arc::new(MailcalApp {
        runtime,
        app,
        account_connect_errors: Mutex::new(account_errors),
        calendar_connect_errors: Mutex::new(calendar_errors),
        registry,
        background,
        mcp,
        mcp_endpoint: Mutex::new(None),
        agent_ui,
        #[cfg(feature = "allodia-license")]
        allodia_tokens: crate::allodia_tokens::Tokens::default(),
        #[cfg(feature = "allodia-license")]
        allodia_health: Mutex::new(crate::AllodiaGrantHealth::Ok),
        allodia_sync: Mutex::new(None),
        allodia: Mutex::new(allodia),
        credential_store,
        disconnected,
        device_zone: device_tz,
        // The real account path: detection reaches the network, as it must.
        showcase: false,
    });
    // Seed the analytics account mix from the registry we just populated. A no-op unless the user
    // has consented, but it must happen before the first event, or a consented install's very
    // first batch would report zero accounts.
    mailcal.refresh_analytics_accounts();
    // Interactive boot: every account is a provider-less placeholder, so dial them all in the
    // background now. Each successful reconnect registers live providers (the cached mail is
    // already on screen), starts that account's IMAP IDLE watches / poll timer, and runs a
    // deferred catch-up sync: so mail then arrives on its own without blocking first paint. It
    // starts hidden and shows progress only if it downloads mail. A headless background-worker
    // build (`new_background_worker`) skips this: it already connected synchronously above and
    // does one bounded `run_background_sync` pass, so standing watches would just waste the OS's
    // brief background window before the process is suspended again.
    if start_live_sync {
        mailcal.retry_connections();
    }
    Ok(mailcal)
}

/// Every stored account, prepared **offline**: the provider-less placeholder the app lists (and
/// paints cached mail for), plus any config too corrupt to derive an id from.
struct Prepared {
    accounts: Vec<BoxedAccount>,
    account_errors: Vec<String>,
}

/// Parses every stored config, builds each account's **one** token source, and registers it; all
/// with **no network at all**.
///
/// This is the step both boot modes now share, and the reason they can: registration is a property
/// of *preparing* an account, not of dialing one, so there is no ordering left for a caller to get
/// wrong. The headless worker used to dial first and register afterwards, which dropped every
/// refresh-token rotation of a cold pass and cost a real account its grant.
///
/// A config too corrupt to derive an id from is recorded as an error and skipped; it cannot be
/// listed at all, and it is never registered, because the parse fails first.
fn prepare_accounts(
    registry: &SharedRegistry,
    sink: &Arc<dyn TokenSink>,
    configs: &[String],
) -> Prepared {
    let mut accounts = Vec::with_capacity(configs.len());
    let mut account_errors = Vec::new();
    for config_toml in configs {
        // Stored: a second core in this process may already hold a fresher token for this
        // account than the store does, and it must be adopted rather than raced.
        match prepare_stored_account(config_toml, sink, CredentialOrigin::Stored) {
            Ok(prepared) => {
                registry.pre_register(prepared.account.id.as_str().to_owned(), prepared.connected);
                accounts.push(prepared.account);
            }
            Err(err) => account_errors.push(err.to_string()),
        }
    }
    log::info!(
        "boot: prepared and registered {} account(s) before any dial",
        accounts.len(),
    );
    Prepared {
        accounts,
        account_errors,
    }
}

/// Routes one dialed account into the app's account list and the two diagnostic channels, keeping a
/// mail failure distinct from a calendar-only one.
///
/// The account is **always** added: a failed dial keeps its placeholder, so an outaged account
/// still lists with a badge instead of vanishing, which is the difference between "my server is
/// down" and "the app lost my account". A mail failure also records its `(id, detail)` in `failed`,
/// which seeds the outage badge and queues the account for reconnect.
///
/// Generic over the account so the classification is unit-testable without a live provider; the
/// one piece of this loop worth pinning on its own, since each channel means something different to
/// a host and they used to be easy to swap.
pub(crate) fn record_dial_outcome<A>(
    account: A,
    id: String,
    failure: Option<DialFailure>,
    accounts: &mut Vec<A>,
    account_errors: &mut Vec<String>,
    calendar_errors: &mut Vec<String>,
    failed: &mut Vec<FailedDial>,
) {
    accounts.push(account);
    match failure {
        Some(DialFailure::MailFailed {
            detail,
            signin_rejected,
        }) => {
            account_errors.push(detail.clone());
            failed.push(FailedDial {
                id,
                detail,
                signin_rejected,
            });
        }
        Some(DialFailure::CalendarOnly(detail)) => calendar_errors.push(detail),
        None => {}
    }
}

/// An account whose boot dial failed: what to tell the user, and **which** of the two things to
/// tell them.
///
/// `signin_rejected` is the same verdict `reconnect_all` branches on, carried here so the two boot
/// modes cannot disagree about what a refusal means: one badging an outage while the other prompts
/// is a difference the user sees and neither mode can detect.
pub(crate) struct FailedDial {
    /// The account id, for the badge/prompt and the reconnect queue.
    pub(crate) id: String,
    /// The account-labelled cause, shown behind the outage badge's "details" link.
    pub(crate) detail: String,
    /// The server refused the account's credential: raise the reconnect prompt instead.
    pub(crate) signin_rejected: bool,
}

/// What went wrong on one account's boot dial, in the two shapes the boot reports differently.
pub(crate) enum DialFailure {
    /// The **mail** connect failed: the account is kept as its placeholder, carrying this
    /// account-labelled detail, and re-dialed later.
    MailFailed {
        /// The account-labelled cause, for the badge's "details" link and the log.
        detail: String,
        /// Whether the server **refused the credential** rather than being unreachable; the
        /// reconnect prompt, not an outage badge (`docs/provider-oauth.md` rule 12).
        signin_rejected: bool,
    },
    /// Mail is up but an optional calendar connect failed; recorded separately so an empty agenda
    /// is not mistaken for a skipped account.
    CalendarOnly(String),
}

/// Dials every prepared account, at most [`MAX_CONCURRENT_DIALS`] at a time, and returns each
/// account (live on success, its original placeholder on failure) beside what went wrong.
///
/// Bounded rather than a plain `join_all`: an unbounded fan-out is what produced bursts of ten and
/// eleven simultaneous connections on a production device right after a network transition. See
/// [`MAX_CONCURRENT_DIALS`].
async fn dial_registered(
    registry: &SharedRegistry,
    placeholders: Vec<BoxedAccount>,
    device_tz: &engine_api::TimeZoneId,
) -> Vec<(BoxedAccount, Option<DialFailure>)> {
    dial_all(placeholders, |index, placeholder| async move {
        let started = Instant::now();
        let id = placeholder.id.clone();
        // Registered a moment ago by `prepare_accounts`, so this is always `Some`, but it is
        // asked rather than assumed, because asking is what makes an unregistered account
        // undialable instead of merely undocumented.
        let Some(dial) = registry.dial(id.as_str()) else {
            log::error!("boot: account[{index}] vanished from the registry before its dial");
            return (placeholder, None);
        };
        let label = dial.label();
        let outcome = dial.run(&id, device_tz.clone()).await;
        let status = match &outcome {
            Ok(_) => "ok",
            // A refused credential is not an outage, and a support log that calls both
            // "unreachable" cannot tell them apart.
            Err(err) if err.signin_expired() => "sign-in REFUSED (kept as placeholder)",
            Err(_) => "unreachable (kept as placeholder)",
        };
        log::info!(
            "boot: account[{index}] connect {status} in {}ms",
            started.elapsed().as_millis(),
        );
        match outcome {
            Ok(outcome) => (
                outcome.account,
                outcome.calendar_error.map(DialFailure::CalendarOnly),
            ),
            Err(err) => (
                placeholder,
                Some(DialFailure::MailFailed {
                    signin_rejected: err.signin_expired(),
                    detail: format!("{label}: {err}"),
                }),
            ),
        }
    })
    .await
}
