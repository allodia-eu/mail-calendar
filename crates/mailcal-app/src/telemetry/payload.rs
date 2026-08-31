//! The telemetry **wire payload**: the device/account context, the batch envelope, and the
//! reducers that coarsen a host's raw facts before any of it leaves the device.
//!
//! Split from [`event`](super::event) (which owns *what happened*) because this module owns
//! *what we are allowed to say about the device it happened on*, and that is the half a reviewer
//! most needs to be able to read in one sitting.
//!
//! Coarsening happens **here, in the core**, not in each client. A client reports the OS version
//! it has and the core reduces it to a major (`15.4.1` → `15`); a client reports its locale and
//! the core reduces it to a language we ship. One tested rule, five clients, and no client can
//! widen the payload by reporting something more precise than we asked for.

use serde::Serialize;

use super::event::{Event, Protocol};

/// The payload schema version. Bumped when the wire shape changes, so the relay rejects a shape
/// it does not know rather than half-parsing it.
pub const SCHEMA: u32 = 1;

/// Every property key that can appear in a payload: the closed set the relay's ingest whitelist
/// mirrors. A key not on this list is a bug here **and** is rejected at the relay, so widening
/// what we send takes two deliberate edits in two repos, never one careless `track()` call.
pub const PROPERTY_KEYS: &[&str] = &[
    // Context; sent once per batch.
    "platform",
    "os_version",
    "device_class",
    "app_version",
    "locale",
    "account_count",
    "has_imap",
    "has_jmap",
    "has_graph",
    "has_google",
    // Per-event.
    "protocol",
    "feature",
    "duration",
    "grouping",
    "quote_style",
    "swipe_left",
    "swipe_right",
];

/// The client platform the events came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl Platform {
    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Ios => "ios",
            Self::Ipados => "ipados",
            Self::Windows => "windows",
            Self::Android => "android",
            Self::Linux => "linux",
        }
    }
}

/// The device's **form factor**; deliberately coarse.
///
/// We do not collect a raw model string (`MacBookPro18,3`, `SM-G991B`). It is the strongest
/// identifier in an otherwise low-entropy payload, and it is what turns a handful of ordinary
/// facts into a fingerprint: with a few thousand installs, a rare model paired with a rare
/// account mix is plausibly a single identifiable person. The app stores already report exact
/// models to us for free and at higher fidelity, so we give up nothing. A class is what actually
/// drives a decision ("does the tablet layout matter?"); the model string never did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// An iPhone.
    Iphone,
    /// An iPad.
    Ipad,
    /// A portable Mac.
    MacLaptop,
    /// A desktop Mac.
    MacDesktop,
    /// A Windows PC. Not split into laptop/desktop: Windows exposes no reliable, permission-free
    /// way to tell, and the split is not worth a permission prompt.
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

impl DeviceClass {
    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Iphone => "iphone",
            Self::Ipad => "ipad",
            Self::MacLaptop => "mac-laptop",
            Self::MacDesktop => "mac-desktop",
            Self::Pc => "pc",
            Self::AndroidPhone => "android-phone",
            Self::AndroidTablet => "android-tablet",
            Self::LinuxDesktop => "linux-desktop",
            Self::Unknown => "unknown",
        }
    }
}

/// The raw device facts a client reports once, at construction. The core **reduces** these before
/// they cross the wire (`Context::build`): a client hands over what it has, and the coarsening
/// rule lives in one tested place rather than six.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The client platform.
    pub platform: Platform,
    /// The OS version as the host reports it (`15.4.1`, `11 (build 22631)`). Reduced to a major
    /// here; never sent raw.
    pub os_version: String,
    /// The device's coarse form factor.
    pub device_class: DeviceClass,
    /// The app's own version (`1.4.0`). Low-entropy (everyone is on one of a handful) and it is
    /// what tells us when an old version can be dropped, so it is sent as-is.
    pub app_version: String,
    /// The host's locale tag (`nl-NL`). Reduced to a bare language we ship.
    pub locale: String,
}

/// The account mix: how many, and which protocol families. Never which addresses, or hosts, or
/// which account is which.
// One independent presence flag per integrated provider: a mix can carry several at once, and
// each maps 1:1 to a boolean key in the wire payload (`PROPERTY_KEYS`), so these are genuinely
// separate bools, not a state to fold into an enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AccountMix {
    /// How many accounts are configured.
    pub(crate) count: usize,
    /// At least one IMAP account.
    pub(crate) has_imap: bool,
    /// At least one JMAP account.
    pub(crate) has_jmap: bool,
    /// At least one Microsoft Graph account.
    pub(crate) has_graph: bool,
    /// At least one Google (Gmail) account.
    pub(crate) has_google: bool,
}

impl AccountMix {
    /// Folds a run of protocols into the mix.
    ///
    /// Deliberately **unordered**: `[Graph, Imap, Imap]` and `[Imap, Graph, Imap]` produce the
    /// same value. An ordered per-account tuple would be markedly higher entropy; it encodes the
    /// shape of one person's specific mail setup, and it answers nothing that "which protocols
    /// are in use" does not.
    pub(crate) fn of(protocols: impl IntoIterator<Item = Protocol>) -> Self {
        let mut mix = Self::default();
        for protocol in protocols {
            mix.count += 1;
            match protocol {
                Protocol::Imap => mix.has_imap = true,
                Protocol::Jmap => mix.has_jmap = true,
                Protocol::Graph => mix.has_graph = true,
                Protocol::Google => mix.has_google = true,
            }
        }
        mix
    }
}

/// The reduced device + account facts, sent **once per batch** rather than stamped onto every
/// event. Every field is an enum label, a bool, or a string that has been through a reducer
/// below.
// The `has_*` provider-presence flags are independent (a mix can carry several) and each is its
// own wire key, so they stay separate bools rather than folding into an enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Context {
    /// The client platform.
    pub platform: &'static str,
    /// The OS **major** version only.
    pub os_version: String,
    /// The device's coarse form factor.
    pub device_class: &'static str,
    /// The app version.
    pub app_version: String,
    /// A language we ship, or `other`.
    pub locale: &'static str,
    /// How many accounts, bucketed.
    pub account_count: &'static str,
    /// At least one IMAP account.
    pub has_imap: bool,
    /// At least one JMAP account.
    pub has_jmap: bool,
    /// At least one Microsoft Graph account.
    pub has_graph: bool,
    /// At least one Google (Gmail) account.
    pub has_google: bool,
}

impl Context {
    /// Reduces the host's raw [`DeviceInfo`] and the current account mix to the wire form.
    pub(crate) fn build(device: &DeviceInfo, accounts: AccountMix) -> Self {
        Self {
            platform: device.platform.as_str(),
            os_version: os_major(&device.os_version),
            device_class: device.device_class.as_str(),
            app_version: device.app_version.clone(),
            locale: locale_tag(&device.locale),
            account_count: account_bucket(accounts.count),
            has_imap: accounts.has_imap,
            has_jmap: accounts.has_jmap,
            has_graph: accounts.has_graph,
            has_google: accounts.has_google,
        }
    }
}

/// Reduces an OS version to its major component: `15.4.1` → `15`, `11 (build 22631)` → `11`.
/// A full build number is a strong identifier and answers no question a major does not. Anything
/// unparseable reduces to `unknown` rather than passing through: the failure mode is losing a
/// data point, never leaking one.
fn os_major(raw: &str) -> String {
    let major: String = raw
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if major.is_empty() {
        "unknown".to_owned()
    } else {
        major
    }
}

/// Reduces a host locale tag to a language we actually ship (`nl-NL` → `nl`). Anything else is
/// `other`: the question is "do we need more languages", not "which of the world's locales is
/// this one person in".
fn locale_tag(raw: &str) -> &'static str {
    match raw.split(['-', '_']).next().unwrap_or_default() {
        "en" => "en",
        "nl" => "nl",
        _ => "other",
    }
}

/// Buckets the account count. A raw count is high-entropy at the tail; "eleven accounts" is a
/// very small population, and the product question is only ever "one, a few, or many".
const fn account_bucket(count: usize) -> &'static str {
    match count {
        0 => "0",
        1 => "1",
        2 => "2",
        3..=5 => "3-5",
        _ => "6+",
    }
}

/// One event as it crosses the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireEvent {
    /// The event name.
    pub name: &'static str,
    /// The event's own properties. Keys are all in [`PROPERTY_KEYS`]; values are all enum labels.
    pub properties: std::collections::BTreeMap<&'static str, String>,
}

/// A batch of events plus the context they share.
///
/// This is the literal body POSTed to the relay **and** the literal JSON the consent screen's
/// "see exactly what we send" panel renders: the same bytes, from the same type, so the preview
/// cannot drift from the reality. That is the point: a user who wants to check what we take can
/// read it, and what they read is true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Batch {
    /// The payload schema version.
    pub schema: u32,
    /// The opaque install id; present only because the user consented.
    pub install_id: String,
    /// The shared device + account context.
    pub context: Context,
    /// The events.
    pub events: Vec<WireEvent>,
}

impl Batch {
    /// Assembles a batch from the consented install id, the reduced context, and some events.
    pub(crate) fn new(install_id: String, context: Context, events: &[Event]) -> Self {
        Self {
            schema: SCHEMA,
            install_id,
            context,
            events: events
                .iter()
                .map(|event| WireEvent {
                    name: event.name(),
                    properties: event.properties(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountMix, Context, DeviceClass, DeviceInfo, Platform};

    #[test]
    fn linux_device_facts_reduce_to_closed_wire_labels() {
        let context = Context::build(
            &DeviceInfo {
                platform: Platform::Linux,
                os_version: "24.04.4 LTS".to_owned(),
                device_class: DeviceClass::LinuxDesktop,
                app_version: "0.2.0".to_owned(),
                locale: "nl_NL.UTF-8".to_owned(),
            },
            AccountMix::default(),
        );

        assert_eq!(context.platform, "linux");
        assert_eq!(context.os_version, "24");
        assert_eq!(context.device_class, "linux-desktop");
        assert_eq!(context.locale, "nl");
    }
}
