//! The in-memory app builders: the demo (a tiny fixture the verify gates assert on) and the
//! showcase (a richer, seeded dataset for store screenshots). Split out of `boot.rs` to keep it
//! under the 500-line limit; neither connects a real account or touches the network, so they
//! share none of the credential/reconnect machinery the real-account boot needs.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use engine_api::{AccountId, EmailAddress, Engine, Provider};
use mailcal_app::{Account, App, Intent as AppIntent, Telemetry, TimeZoneInit};
use mailcal_viewmodel::SignatureSlotKind;

use crate::{
    LogLevel, Logger, MailcalApp, Observer,
    background::BackgroundManager,
    demo::DemoProvider,
    device_zone, logging,
    observer::{DebouncedObserver, ObserverBridge},
    runtime,
    showcase::{ShowcaseCalendarProvider, ShowcaseMailProvider},
    showcase_contacts::ShowcaseContactsProvider,
    showcase_data::{self, ShowcaseLocale},
};

/// Builds an in-memory demo app: the body of [`MailcalApp::new_demo`].
pub(crate) fn build_demo(
    observer: Box<dyn Observer>,
    logger: Box<dyn Logger>,
    log_level: LogLevel,
    device_timezone: String,
) -> Arc<MailcalApp> {
    logging::install_logger(logger, log_level);
    log::info!("demo app starting (in-memory engine)");
    let device_tz = device_zone(device_timezone);
    let engine = Engine::open_in_memory().expect("in-memory engine opens");
    let account = Account {
        id: AccountId::try_from("demo").expect("valid account id"),
        providers: vec![Box::new(DemoProvider::new()) as Box<dyn Provider>],
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("demo@allodia.local"),
    };

    let app = App::new(
        engine,
        vec![account],
        TimeZoneInit {
            device_zone: device_tz.clone(),
            prefs_path: None,
        },
        // The demo has no real account to connect, so no on-demand connector.
        None,
        std::sync::Arc::new(DebouncedObserver::new(ObserverBridge { foreign: observer })),
        // No device, no sink, no preferences file: the demo cannot phone home even if a consent
        // decision were somehow recorded in it.
        Telemetry::off(None),
    );
    let runtime = runtime();
    let app = Arc::new(app);
    let registry = crate::account_registry::AccountRegistry::new();
    // The demo has no real accounts to push/poll, so the manager is built but never started.
    let background = Arc::new(BackgroundManager::new(
        Arc::clone(&app),
        Arc::clone(&registry),
        runtime.handle().clone(),
    ));
    // A real server object, so the type is uniform across every build, but one that is never
    // given an endpoint, and therefore cannot listen whatever a setting says.
    let agent_ui: crate::agent_ui::AgentUiSlot = Arc::new(Mutex::new(None));
    let mcp = Arc::new(mailcal_mcp::McpServer::new(
        Arc::new(crate::mcp::AppBackend::new(
            Arc::clone(&app),
            Arc::clone(&agent_ui),
        )),
        runtime.handle().clone(),
    ));
    Arc::new(MailcalApp {
        runtime,
        app,
        account_connect_errors: Mutex::new(Vec::new()),
        calendar_connect_errors: Mutex::new(Vec::new()),
        registry,
        background,
        mcp,
        mcp_endpoint: Mutex::new(None),
        agent_ui,
        // A bundled-fixture app signs in to nothing.
        #[cfg(feature = "allodia-license")]
        allodia_tokens: crate::allodia_tokens::Tokens::default(),
        #[cfg(feature = "allodia-license")]
        allodia_health: Mutex::new(crate::AllodiaGrantHealth::Ok),
        allodia_sync: Mutex::new(None),
        allodia: Mutex::new(None),
        credential_store: Arc::new(crate::credential_store::NoStoredCredentials),
        // The demo connects no real accounts, so nothing is ever disconnected.
        disconnected: Arc::new(Mutex::new(HashSet::new())),
        device_zone: device_tz,
        // The demo fixture is what the CI verify gates assert on; it must keep behaving like a
        // real app, detection included.
        showcase: false,
    })
}

/// Builds an in-memory **showcase** app: the body of [`MailcalApp::new_showcase`]. Two fictional
/// accounts over one ephemeral engine (a primary mailbox with folders, a threaded conversation, an
/// attachment and a calendar; a lighter mail-only second so the unified inbox and switcher look
/// real), served from bundled `locale` sample content, nothing persisted or connected.
pub(crate) fn build_showcase(
    observer: Box<dyn Observer>,
    logger: Box<dyn Logger>,
    log_level: LogLevel,
    device_timezone: String,
    locale: ShowcaseLocale,
) -> Arc<MailcalApp> {
    logging::install_logger(logger, log_level);
    log::info!(
        "showcase (screenshot) app starting (in-memory engine, seeded {locale:?} sample content)"
    );
    let device_tz = device_zone(device_timezone);
    let engine = Engine::open_in_memory().expect("in-memory engine opens");
    // A pinned wall clock, not the moment of capture: a screenshot that differs only because
    // time passed is republished for nothing, and hides the changes that matter.
    let now = showcase_data::seeded_now(&device_tz);
    let primary = showcase_data::primary(locale, now);
    let (calendars, events) = showcase_data::primary_calendar(locale, now);
    let primary_account = Account {
        id: AccountId::try_from(primary.identity.as_str()).expect("valid account id"),
        providers: vec![Box::new(ShowcaseMailProvider::new(
            primary.mailboxes,
            primary.messages,
            primary.bodies,
        )) as Box<dyn Provider>],
        calendar_providers: vec![
            Box::new(ShowcaseCalendarProvider::new(calendars, events)) as Box<dyn Provider>
        ],
        contact_providers: vec![Box::new(ShowcaseContactsProvider::new(
            crate::showcase_contacts::primary_contacts(),
        )) as Box<dyn engine_api::ContactsProvider>],
        identity: EmailAddress::new(primary.identity.clone()),
    };

    let secondary = showcase_data::secondary(locale, now);
    // Held before the seeds are moved into the Account values below, so the signature
    // assignment can name each account afterwards.
    let primary_identity = primary_account.id.as_str().to_owned();
    let secondary_account = Account {
        id: AccountId::try_from(secondary.identity.as_str()).expect("valid account id"),
        providers: vec![Box::new(ShowcaseMailProvider::new(
            secondary.mailboxes,
            secondary.messages,
            secondary.bodies,
        )) as Box<dyn Provider>],
        // The second account is mail-only, so the calendar comes solely from the primary.
        calendar_providers: Vec::new(),
        // It does carry contacts, though; one of them the same person as the primary's, which
        // is what makes the merged "in 2 accounts" row visible in the showcase.
        contact_providers: vec![Box::new(ShowcaseContactsProvider::new(
            crate::showcase_contacts::secondary_contacts(),
        )) as Box<dyn engine_api::ContactsProvider>],
        identity: EmailAddress::new(secondary.identity.clone()),
    };
    let secondary_identity = secondary_account.id.as_str().to_owned();

    let app = App::new(
        engine,
        vec![primary_account, secondary_account],
        TimeZoneInit {
            device_zone: device_tz.clone(),
            prefs_path: None,
        },
        // Everything is served from one account-wide snapshot, so no on-demand connector.
        None,
        std::sync::Arc::new(DebouncedObserver::new(ObserverBridge { foreign: observer })),
        // Screenshot mode: no device, no sink, no preferences file; it cannot phone home.
        Telemetry::off(None),
    );
    let runtime = runtime();
    let app = Arc::new(app);
    // Prime the seeded inbox synchronously (like the real path's prime_snapshot) so the host
    // paints rows on connect, not racing a post-boot RefreshMail; Android came up blank.
    runtime.block_on(app.dispatch(AppIntent::RefreshMail));
    // …and the calendar with it, which the real path gets from `prime_calendar` reading the
    // on-disk store. This boot has no store to prime from, so the equivalent is one sync over the
    // in-memory provider; microseconds, and it is what makes the *invitation* card honest: the
    // card says "we have not looked at your calendar" (and hides its day preview) until the
    // calendar cache actually covers the meeting's day, and mail otherwise syncs first. Without
    // this the invitation screenshot would show a card with no picture under it.
    runtime.block_on(app.dispatch(AppIntent::RefreshCalendar));
    // Seed the signature library and point both slots of each account at its own signature.
    // Persistence is off in this boot (`prefs_path: None`), so this lives only for the run;
    // which is exactly right for a screenshot dataset. Without it the composer's Signature
    // control never appears (it is hidden while the library is empty) and the Settings category
    // shows its empty state, so a store capture would advertise neither.
    runtime.block_on(seed_signatures(
        &app,
        locale,
        &primary_identity,
        &secondary_identity,
    ));
    let registry = crate::account_registry::AccountRegistry::new();
    let background = Arc::new(BackgroundManager::new(
        Arc::clone(&app),
        Arc::clone(&registry),
        runtime.handle().clone(),
    ));
    // A real server object, so the type is uniform across every build, but one that is never
    // given an endpoint, and therefore cannot listen whatever a setting says.
    let agent_ui: crate::agent_ui::AgentUiSlot = Arc::new(Mutex::new(None));
    let mcp = Arc::new(mailcal_mcp::McpServer::new(
        Arc::new(crate::mcp::AppBackend::new(
            Arc::clone(&app),
            Arc::clone(&agent_ui),
        )),
        runtime.handle().clone(),
    ));
    Arc::new(MailcalApp {
        runtime,
        app,
        account_connect_errors: Mutex::new(Vec::new()),
        calendar_connect_errors: Mutex::new(Vec::new()),
        registry,
        background,
        mcp,
        mcp_endpoint: Mutex::new(None),
        agent_ui,
        // A bundled-fixture app signs in to nothing.
        #[cfg(feature = "allodia-license")]
        allodia_tokens: crate::allodia_tokens::Tokens::default(),
        #[cfg(feature = "allodia-license")]
        allodia_health: Mutex::new(crate::AllodiaGrantHealth::Ok),
        allodia_sync: Mutex::new(None),
        allodia: Mutex::new(None),
        credential_store: Arc::new(crate::credential_store::NoStoredCredentials),
        // The showcase connects no real accounts, so nothing is ever disconnected.
        disconnected: Arc::new(Mutex::new(HashSet::new())),
        device_zone: device_tz,
        // The one build where account detection answers from a script instead of the network;
        // see `MailcalApp::detect_account_settings`.
        showcase: true,
    })
}

/// Seeds the showcase signature library: one signature per account, each pointed at by **both**
/// of that account's slots, so a new message and a reply/forward both open with one.
///
/// Assigning both slots is what makes the screenshots honest rather than lucky: the reply
/// capture composes from the primary account, and a `NewMessage`-only assignment would leave it
/// with no signature while the Settings screen said it had one.
async fn seed_signatures(
    app: &App<Box<dyn Provider>>,
    locale: ShowcaseLocale,
    primary: &str,
    secondary: &str,
) {
    let seeds = showcase_data::signatures(locale);
    for (account, seed) in [(primary, &seeds.primary), (secondary, &seeds.secondary)] {
        let row = app
            .create_signature(
                seed.name.to_owned(),
                seed.body_html.to_owned(),
                seed.body_plain.to_owned(),
            )
            .await;
        for slot in [
            SignatureSlotKind::NewMessage,
            SignatureSlotKind::ReplyForward,
        ] {
            app.set_account_signature(account, slot, Some(row.id.clone()))
                .await;
        }
    }
}
