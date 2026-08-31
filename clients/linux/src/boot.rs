//! Constructs the shared core for the Linux host.

#[cfg(debug_assertions)]
mod dev;

use std::{fs, path::PathBuf, sync::Arc};

#[cfg(debug_assertions)]
use dev::{dev_credential_store, dev_secrets, with_stored_allodia_account};
use mailcal_bindings::{
    AccountCredentialStore, DeviceClass, DeviceInfo, LogLevel, MailcalApp, Observer, Platform,
    device_time_zone,
};

use crate::{
    logger::FileLogger,
    preferences,
    secrets::{SecretSink, SecretStore},
};

/// The shared app plus production-only host services used after construction.
pub(crate) struct BootedApp {
    pub(crate) app: Arc<MailcalApp>,
    pub(crate) secrets: Option<Arc<SecretStore>>,
    /// Whether this launch may keep its account list in step with the person's other devices.
    ///
    /// False for the in-memory demo and showcase, which have no accounts worth syncing, and for a
    /// dev-account launch, whose accounts belong to the local test server; sending those up would
    /// put a harness mailbox on the developer's own phone.
    pub(crate) syncable: bool,
}

impl std::fmt::Debug for BootedApp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BootedApp")
            .field("has_secrets", &self.secrets.is_some())
            .finish_non_exhaustive()
    }
}

/// Opens the debug namespace a fixture launch keeps its own credentials in.
///
/// These launches connect canned accounts rather than the developer's, so for a long time they
/// were handed a store that refused every write; the safe answer while the only thing that could
/// have been written was a fixture with no grant behind it. Signing in to an **Allodia account**
/// is not that: it is a real grant, obtained by a real person, and refusing to keep it made the
/// harness the one mode where signing in never stuck.
///
/// So the fixture launches get a real store on a namespace of their own, keyed by the dev account
/// (`dev`, `dev-imap`, `dev-multi`), the shape the Windows client already uses. Nothing a harness
/// run writes can land among the real accounts, and nothing it reads can see them.
///
/// A keyring that will not open is **not** fatal here, unlike the production path: a fixture
/// launch has canned accounts to show either way, and a developer without a working Secret Service
/// should still get an app. The sign-in card reports the refusal when they try to use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BootMode {
    Accounts,
    #[cfg(debug_assertions)]
    Demo,
    #[cfg(debug_assertions)]
    StalwartJmap,
    /// The same harness over JMAP as **two** accounts ([`crate::dev_account`]).
    #[cfg(debug_assertions)]
    StalwartMulti,
    #[cfg(debug_assertions)]
    StalwartImap,
    /// The seeded, offline screenshot dataset ([`crate::showcase`]).
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    Showcase,
}

/// Builds either the Secret Service-backed production shape or a debug-only deterministic provider.
///
/// **This is also where diagnostics begin, and everything before it is deliberately uncovered.**
/// The core installs the `log` sink and the panic hook as it constructs the app, so a panic or a
/// GTK critical raised earlier: reading preferences, `set_locale_override`, GTK's own start-up,
/// building the window root; reaches the file log nowhere. Hoisting the sink into a global so
/// `main` could arm a hook first was considered and rejected: the window it would cover is exactly
/// the window in which there is no window, so the user cannot reach Settings → Diagnostics to hand
/// the file over, and a developer with shell access already has stderr and the journal, where the
/// default GLib handler and the default panic hook both still print. It would buy a line nobody
/// can retrieve, at the cost of a second logger instance over one file.
pub(crate) fn app(observer: Box<dyn Observer>) -> Result<BootedApp, String> {
    let logger = Box::new(FileLogger::new());
    let timezone = device_time_zone();
    let log_level = if preferences::global().diagnostics_debug() {
        LogLevel::Debug
    } else {
        LogLevel::Info
    };
    match boot_mode()? {
        BootMode::Accounts => {
            let secrets = Arc::new(SecretStore::open()?);
            let configs = secrets.configs()?;
            let app = real_app(
                observer,
                logger,
                log_level,
                timezone,
                configs,
                None,
                Box::new(SecretSink::new(Arc::clone(&secrets))),
            )?;
            Ok(BootedApp {
                app,
                secrets: Some(secrets),
                syncable: true,
            })
        }
        #[cfg(debug_assertions)]
        BootMode::Demo => Ok(BootedApp {
            app: MailcalApp::new_demo(observer, logger, log_level, timezone),
            secrets: None,
            syncable: false,
        }),
        // In-memory and offline, so it needs neither a data directory nor a credential store,
        // and must not be handed one, because nothing a screenshot run does should reach the
        // developer's keyring.
        #[cfg(any(debug_assertions, feature = "dev-harness"))]
        BootMode::Showcase => Ok(BootedApp {
            app: MailcalApp::new_showcase(
                observer,
                logger,
                log_level,
                timezone,
                crate::showcase::seed_locale(),
            ),
            secrets: None,
            syncable: false,
        }),
        #[cfg(debug_assertions)]
        BootMode::StalwartJmap => {
            let config =
                crate::dev_account::stalwart_jmap_toml().map_err(|error| error.to_string())?;
            let secrets = dev_secrets("dev");
            let app = real_app(
                observer,
                logger,
                log_level,
                timezone,
                with_stored_allodia_account(vec![config], secrets.as_ref()),
                Some("dev"),
                dev_credential_store(secrets.as_ref()),
            )?;
            Ok(BootedApp {
                app,
                secrets,
                syncable: false,
            })
        }
        // Its own store (`dev-multi`), like every other fixture: two harness accounts sharing the
        // single-account store would leave bob's mail behind in it for the next `stalwart` boot.
        #[cfg(debug_assertions)]
        BootMode::StalwartMulti => {
            let first =
                crate::dev_account::stalwart_jmap_toml().map_err(|error| error.to_string())?;
            let second = crate::dev_account::stalwart_jmap_toml_second()
                .map_err(|error| error.to_string())?;
            let secrets = dev_secrets("dev-multi");
            let app = real_app(
                observer,
                logger,
                log_level,
                timezone,
                with_stored_allodia_account(vec![first, second], secrets.as_ref()),
                Some("dev-multi"),
                dev_credential_store(secrets.as_ref()),
            )?;
            Ok(BootedApp {
                app,
                secrets,
                syncable: false,
            })
        }
        #[cfg(debug_assertions)]
        BootMode::StalwartImap => {
            let secrets = dev_secrets("dev-imap");
            let app = real_app(
                observer,
                logger,
                log_level,
                timezone,
                with_stored_allodia_account(
                    vec![crate::dev_account::STALWART_IMAP_TOML.to_owned()],
                    secrets.as_ref(),
                ),
                Some("dev-imap"),
                dev_credential_store(secrets.as_ref()),
            )?;
            Ok(BootedApp {
                app,
                secrets,
                syncable: false,
            })
        }
    }
}

fn real_app(
    observer: Box<dyn Observer>,
    logger: Box<FileLogger>,
    log_level: LogLevel,
    timezone: String,
    configs: Vec<String>,
    data_subdir: Option<&str>,
    credential_store: Box<dyn AccountCredentialStore>,
) -> Result<Arc<MailcalApp>, String> {
    let data_dir = data_dir(data_subdir);
    fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
    let app = MailcalApp::new_accounts(
        observer,
        logger,
        log_level,
        configs,
        data_dir.to_string_lossy().into_owned(),
        timezone,
        device_info(),
        credential_store,
    )
    .map_err(|error| error.to_string())?;
    // Where this device remembers what it has synced with the account service, beside the engine
    // store so a dev launch's bookkeeping is isolated with the accounts it describes. A store that
    // cannot be read leaves syncing off for this launch rather than starting from nothing, which
    // would re-adopt every record and re-offer every account.
    if let Err(error) = app.use_allodia_sync_state_store(Box::new(
        crate::allodia_sync_store::FileSyncStateStore::in_data_dir(&data_dir),
    )) {
        log::warn!("allodia: the sync state could not be read ({error}); not syncing this launch");
    }
    Ok(app)
}

fn data_dir(subdir: Option<&str>) -> PathBuf {
    let root = gtk::glib::user_data_dir().join("mailcal");
    subdir.map_or(root.clone(), |name| root.join(name))
}

fn device_info() -> DeviceInfo {
    DeviceInfo {
        platform: Platform::Linux,
        os_version: os_version(),
        device_class: DeviceClass::LinuxDesktop,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        locale: sys_locale::get_locale().unwrap_or_else(|| "en".to_owned()),
    }
}

fn os_version() -> String {
    fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("VERSION_ID=")
                    .map(|value| value.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(debug_assertions)]
fn boot_mode() -> Result<BootMode, String> {
    resolve_boot_mode(
        std::env::var("MAILCAL_DEV_ACCOUNT").ok().as_deref(),
        crate::showcase::is_on(),
    )
}

#[cfg(all(not(debug_assertions), feature = "dev-harness"))]
fn boot_mode() -> Result<BootMode, String> {
    if crate::showcase::is_on() {
        return Ok(BootMode::Showcase);
    }
    match std::env::var("MAILCAL_DEV_ACCOUNT").ok().as_deref() {
        None | Some("personal") => Ok(BootMode::Accounts),
        Some(other) => Err(format!(
            "MAILCAL_DEV_ACCOUNT={other} is unavailable in an optimized fixture build; use showcase"
        )),
    }
}

#[cfg(not(any(debug_assertions, feature = "dev-harness")))]
const fn boot_mode() -> Result<BootMode, String> {
    Ok(BootMode::Accounts)
}

/// Refuses a fixture this client cannot boot, so `main` can exit before the window exists.
#[cfg(any(debug_assertions, feature = "dev-harness"))]
pub(crate) fn check_dev_account() -> Result<(), String> {
    boot_mode().map(|_| ())
}

/// Showcase wins over a dev account, because the two say different things and only one of them is
/// safe to get wrong: a capture run that quietly opened the harness would photograph a mailbox
/// nobody wrote the copy for, and one that quietly opened `personal` would photograph real mail.
///
/// An unrecognised value is an **error**, never a fall-through to the stored accounts. The switch
/// is offered by `scripts/dev/boot.sh` for every platform, so a mode this client has not
/// implemented arrives here routinely; and answering it by opening the developer's own mailbox
/// looks, to the operator who asked for a harness, exactly like a harness that seeded nothing.
#[cfg(debug_assertions)]
fn resolve_boot_mode(value: Option<&str>, showcase: bool) -> Result<BootMode, String> {
    if showcase {
        return Ok(BootMode::Showcase);
    }
    match value {
        None | Some("personal") => Ok(BootMode::Accounts),
        Some("demo") => Ok(BootMode::Demo),
        Some("stalwart") => Ok(BootMode::StalwartJmap),
        Some("stalwart-multi") => Ok(BootMode::StalwartMulti),
        Some("stalwart-imap") => Ok(BootMode::StalwartImap),
        Some(other) => Err(format!(
            "MAILCAL_DEV_ACCOUNT={other} is not a fixture this client can boot \
             (demo, stalwart, stalwart-multi, stalwart-imap, personal)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{BootMode, resolve_boot_mode};

    #[test]
    fn only_known_debug_values_select_fixture_accounts() {
        assert_eq!(resolve_boot_mode(Some("demo"), false), Ok(BootMode::Demo));
        assert_eq!(
            resolve_boot_mode(Some("stalwart"), false),
            Ok(BootMode::StalwartJmap)
        );
        assert_eq!(
            resolve_boot_mode(Some("stalwart-multi"), false),
            Ok(BootMode::StalwartMulti)
        );
        assert_eq!(
            resolve_boot_mode(Some("stalwart-imap"), false),
            Ok(BootMode::StalwartImap)
        );
    }

    /// Only an explicit request opens the developer's own mailbox.
    #[test]
    fn the_stored_accounts_need_asking_for() {
        assert_eq!(resolve_boot_mode(None, false), Ok(BootMode::Accounts));
        assert_eq!(
            resolve_boot_mode(Some("personal"), false),
            Ok(BootMode::Accounts)
        );
    }

    /// `boot.sh` offers every mode on every platform, so a fixture this client has not implemented
    /// arrives here in normal use. Answering it with the stored accounts shows the operator their
    /// real mail under the impression it is a harness seeded with nothing.
    #[test]
    fn an_unimplemented_fixture_refuses_instead_of_opening_real_accounts() {
        let refusal = resolve_boot_mode(Some("stalwart-quad"), false)
            .expect_err("an unimplemented fixture must not resolve to a boot mode");
        assert!(refusal.contains("stalwart-quad"), "{refusal}");
        assert!(refusal.contains("stalwart-multi"), "{refusal}");
    }

    #[test]
    fn showcase_outranks_every_dev_account_including_personal() {
        assert_eq!(resolve_boot_mode(None, true), Ok(BootMode::Showcase));
        assert_eq!(
            resolve_boot_mode(Some("stalwart"), true),
            Ok(BootMode::Showcase)
        );
        // The one that matters: a capture run must never open the developer's real accounts.
        assert_eq!(
            resolve_boot_mode(Some("personal"), true),
            Ok(BootMode::Showcase)
        );
    }
}
