//! The consented-analytics FFI: the consent question, the withdrawal switch, the payload
//! preview, and the device facts a host reports.
//!
//! # Why the relay URL is a *build-time* constant
//!
//! [`relay_config`] reads `ALLODIA_TELEMETRY_URL` with `option_env!`, so it is baked in at
//! compile time or not at all. Two consequences, both deliberate:
//!
//! - **A plain `cargo build` has no telemetry endpoint**, so it can never send anything. The
//!   harness, the demo, the showcase, and every developer's local build are silent by construction,
//!   not by remembering to switch something off.
//! - **The host cannot choose the destination.** It is not a constructor argument, so no client;
//!   and nothing a client is tricked into loading; can point the stream somewhere else. The only
//!   endpoint is the one release CI bakes in.
//!
//! # Why `DeviceInfo` is a constructor argument and not a callback port
//!
//! It is a one-shot value, like the `device_timezone` string next to it: the host reports it
//! once and it never changes. A `callback_interface` would need a stub implementation in **both**
//! `MailcalVerify` gates (Swift on macOS CI, C# on Windows CI) and would let the host be asked
//! for device facts at arbitrary later moments. A plain record is smaller, dumber, and
//! auditable: whatever the host passes at boot is all we will ever have.

use std::path::PathBuf;

use mailcal_app::Telemetry;
use mailcal_telemetry::{HttpTelemetrySink, RelayConfig};

use crate::MailcalApp;

/// The relay this build reports to, or `None` (the default) for a build with no endpoint baked
/// in, which never sends anything.
fn relay_config() -> Option<RelayConfig> {
    let base_url = option_env!("ALLODIA_TELEMETRY_URL")?;
    if base_url.is_empty() {
        return None;
    }
    Some(RelayConfig {
        base_url: base_url.to_owned(),
        // Not a secret; it ships in the binary and anyone can read it. It lets the relay tell
        // this product's events from another Allodia product's, and lets us turn one product's
        // ingest off without a client release. The credential that *is* secret lives on the relay.
        app_key: option_env!("ALLODIA_TELEMETRY_APP_KEY")
            .unwrap_or("mailcal")
            .to_owned(),
    })
}

/// Builds the analytics wiring for a **real** app.
///
/// Falls back to [`Telemetry::unsent`] when this build has no relay baked in, or when the sink
/// cannot be constructed; **not** to `Telemetry::off`, which is the demo/showcase shape and
/// throws the device facts away. The difference is user-visible: consent is recorded either way,
/// so the welcome screen and the Settings toggle work identically in a local build, and the
/// "see exactly what we send" panel must show the payload this device would *really* produce.
/// A build with no relay has nowhere to send; it does not have nothing to say.
///
/// Analytics failing to initialise must never stop the app from starting.
///
/// Must be called from inside the app's tokio runtime (the sink spawns its delivery worker).
pub(crate) fn build_telemetry(prefs_path: PathBuf, device: DeviceInfo) -> Telemetry {
    let Some(config) = relay_config() else {
        log::info!("analytics: no relay endpoint in this build, nothing will be sent");
        return Telemetry::unsent(Some(prefs_path), device.into());
    };
    match HttpTelemetrySink::new(config) {
        Ok(sink) => Telemetry::new(Some(prefs_path.clone()), device.into(), Box::new(sink)),
        Err(err) => {
            log::warn!(
                "analytics: could not build the telemetry sink ({err}), nothing will be sent"
            );
            Telemetry::unsent(Some(prefs_path), device.into())
        }
    }
}

/// The client platform, as the host reports it.
#[derive(uniffi::Enum, Clone, Copy, Debug)]
pub enum Platform {
    /// macOS.
    Macos,
    /// iPhone.
    Ios,
    /// iPad.
    Ipados,
    /// Windows.
    Windows,
    /// Android.
    Android,
    /// Linux.
    Linux,
}

/// The device's coarse form factor.
///
/// A **class**, never a model string. `MacBookPro18,3` or `SM-G991B` is the strongest identifier
/// an otherwise low-entropy payload could carry; with a few thousand installs, a rare model
/// paired with a rare account mix is plausibly one identifiable person, and the app stores
/// already report exact models to us for free. A class is what actually drives a decision.
#[derive(uniffi::Enum, Clone, Copy, Debug)]
pub enum DeviceClass {
    /// An iPhone.
    Iphone,
    /// An iPad.
    Ipad,
    /// A portable Mac.
    MacLaptop,
    /// A desktop Mac.
    MacDesktop,
    /// A Windows PC.
    Pc,
    /// An Android phone.
    AndroidPhone,
    /// An Android tablet.
    AndroidTablet,
    /// A Linux desktop or laptop.
    LinuxDesktop,
    /// The host could not classify the device.
    Unknown,
}

/// The device facts a host reports once, at construction.
///
/// Report them **raw**: the full OS version, the host's own locale tag. The core coarsens them
/// (`15.4.1` → `15`, `nl-NL` → `nl`) before anything crosses the wire, so the reduction rule
/// lives in one tested place rather than being reimplemented in six clients, and no client can
/// widen the payload by reporting something more precise than we asked for.
#[derive(uniffi::Record, Clone, Debug)]
pub struct DeviceInfo {
    /// The client platform.
    pub platform: Platform,
    /// The OS version as the OS reports it.
    pub os_version: String,
    /// The device's coarse form factor.
    pub device_class: DeviceClass,
    /// The app's own version (`1.4.0`).
    pub app_version: String,
    /// The host's locale tag (`nl-NL`).
    pub locale: String,
}

/// The device facts the FFI tests boot with. Analytics is off in a test build anyway (no relay is
/// baked in), so this only has to satisfy the constructor.
#[cfg(test)]
pub(crate) fn test_device() -> DeviceInfo {
    DeviceInfo {
        platform: Platform::Macos,
        os_version: "15.0".to_owned(),
        device_class: DeviceClass::MacLaptop,
        app_version: "0.0.0".to_owned(),
        locale: "en".to_owned(),
    }
}

/// Whether the user has been asked about analytics, and whether it is on.
#[derive(uniffi::Record, Clone, Copy, Debug)]
pub struct AnalyticsConsent {
    /// Whether we have asked. `false` on a first launch; show the consent screen. Once asked, we
    /// do not ask again (a decline is remembered) unless the notice materially changes.
    pub asked: bool,
    /// Whether analytics is on. `false` unless the user actively opted in.
    pub enabled: bool,
}

impl From<Platform> for mailcal_app::Platform {
    fn from(platform: Platform) -> Self {
        match platform {
            Platform::Macos => Self::Macos,
            Platform::Ios => Self::Ios,
            Platform::Ipados => Self::Ipados,
            Platform::Windows => Self::Windows,
            Platform::Android => Self::Android,
            Platform::Linux => Self::Linux,
        }
    }
}

impl From<DeviceClass> for mailcal_app::DeviceClass {
    fn from(class: DeviceClass) -> Self {
        match class {
            DeviceClass::Iphone => Self::Iphone,
            DeviceClass::Ipad => Self::Ipad,
            DeviceClass::MacLaptop => Self::MacLaptop,
            DeviceClass::MacDesktop => Self::MacDesktop,
            DeviceClass::Pc => Self::Pc,
            DeviceClass::AndroidPhone => Self::AndroidPhone,
            DeviceClass::AndroidTablet => Self::AndroidTablet,
            DeviceClass::LinuxDesktop => Self::LinuxDesktop,
            DeviceClass::Unknown => Self::Unknown,
        }
    }
}

impl From<DeviceInfo> for mailcal_app::DeviceInfo {
    fn from(device: DeviceInfo) -> Self {
        Self {
            platform: device.platform.into(),
            os_version: device.os_version,
            device_class: device.device_class.into(),
            app_version: device.app_version,
            locale: device.locale,
        }
    }
}

impl From<mailcal_app::AnalyticsConsent> for AnalyticsConsent {
    fn from(consent: mailcal_app::AnalyticsConsent) -> Self {
        Self {
            asked: consent.asked,
            enabled: consent.enabled,
        }
    }
}

impl MailcalApp {
    /// Pushes the current account→protocol map down to the core, which folds it into the
    /// bucketed count + protocol booleans the payload carries.
    ///
    /// Called from **every** place the account registry is written; boot, add, remove, Microsoft
    /// sign-in: so the mix cannot go stale. One function, so the mapping cannot drift between
    /// call sites.
    ///
    /// The account **ids** stay on this side: the core keys protocol lookups by them, and they
    /// never reach the payload (an id embeds the address).
    pub(crate) fn refresh_analytics_accounts(&self) {
        self.app.set_accounts(self.registry.protocols());
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Whether the user has been asked about analytics, and whether it is on.
    ///
    /// The host pulls this once at boot. `asked == false` means show the consent screen; do that
    /// before account setup, and before the OS notification prompt so two asks don't stack.
    pub fn analytics_consent(&self) -> AnalyticsConsent {
        self.app.analytics_consent().into()
    }

    /// Records the user's analytics decision. **Default off**; call this with `true` only from a
    /// deliberate, affirmative action (a switch the user moved, a button they pressed), never
    /// from a pre-checked box, an implied consent, or acceptance of terms.
    ///
    /// Opting in mints the install id. Opting out clears it and asks the backend to erase
    /// everything held under it (GDPR Art. 17).
    pub fn set_analytics_consent(&self, enabled: bool) {
        self.app.set_analytics_consent(enabled);
    }

    /// The **exact JSON** this install would send, pretty-printed.
    ///
    /// For the consent screen's "see exactly what we send" panel and the Settings screen. Built
    /// from the same type the sink serializes, so what the user reads is what actually goes on the
    /// wire: a claim we can make because it is structurally true, not because we checked once.
    pub fn analytics_payload_preview(&self) -> String {
        self.app.analytics_payload_preview()
    }

    /// Reports a launch: the retention signal plus a snapshot of the user's settings. The host
    /// calls this once per launch, after boot. A no-op until the user opts in.
    pub fn report_app_opened(&self) {
        self.app.report_app_opened();
    }
}
