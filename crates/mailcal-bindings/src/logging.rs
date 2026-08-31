//! The FFI **logging port**: a host-injected sink every layer's diagnostics flow to.
//!
//! The product core (and this binding layer) emit records through the lightweight [`log`]
//! facade (`log::info!`, `log::debug!`, …) with no knowledge of where they go. A host
//! passes a [`Logger`] in at construction ([`MailcalApp::new_accounts`](crate::MailcalApp));
//! [`install_logger`] wires a [`LogBridge`] as the process logger so every `log` record is
//! forwarded to that one sink: the Windows file log, Apple's `os_log`, Android's Logcat.
//! One log, every layer, so a field issue is diagnosed from a single stream.
//!
//! The host controls verbosity with a [`LogLevel`]: the default keeps the volume low
//! (info/warn/error), and a debug build (or a support session) can opt into the granular
//! `debug`/`trace` timing without a rebuild via [`MailcalApp::set_log_level`](crate::MailcalApp).
//! Filtering happens in the `log` macros against the global max level, so a suppressed record
//! costs nothing and never crosses the FFI.
//!
//! Privacy: the core logs counts, durations, ids, and high-level events; never mail/event
//! content, addresses, or credentials: so this stream is safe to surface to a user.

use std::sync::{OnceLock, RwLock};

/// The severity of a log record, mirroring [`log::Level`] across the FFI so a host can map
/// it to its native logger's levels (and gate its own sink if it wants).
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// A failure the user may need to know about.
    Error,
    /// Something unexpected that did not stop the operation.
    Warn,
    /// A high-level lifecycle event (boot phases, sync summaries): the default ceiling.
    Info,
    /// Granular diagnostics, including per-phase timing; opt-in.
    Debug,
    /// The most verbose tracing; opt-in.
    Trace,
}

impl From<log::Level> for LogLevel {
    fn from(level: log::Level) -> Self {
        match level {
            log::Level::Error => Self::Error,
            log::Level::Warn => Self::Warn,
            log::Level::Info => Self::Info,
            log::Level::Debug => Self::Debug,
            log::Level::Trace => Self::Trace,
        }
    }
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
            LogLevel::Trace => Self::Trace,
        }
    }
}

/// A foreign (C#/Swift/Kotlin) logging sink the core forwards every diagnostic to. The host
/// implements it over its platform-native logger so all layers land in one log. Must be
/// cheap and non-blocking; it is called on the runtime's worker threads. `target` is the
/// emitting module path (e.g. `mailcal_app::sync`), useful for filtering at the sink.
#[uniffi::export(callback_interface)]
pub trait Logger: Send + Sync {
    /// Records one message at `level` from `target`. The message holds no sensitive content.
    fn log(&self, level: LogLevel, target: String, message: String);
}

/// The process-wide sink the [`LogBridge`] forwards to. Behind an [`RwLock`] so a later
/// constructor (a second `MailcalApp`, or the demo after the real app) can swap the host
/// logger without re-registering the global `log` logger (which can be set only once).
fn sink() -> &'static RwLock<Option<Box<dyn Logger>>> {
    static SINK: OnceLock<RwLock<Option<Box<dyn Logger>>>> = OnceLock::new();
    SINK.get_or_init(|| RwLock::new(None))
}

/// The [`log::Log`] implementation registered once as the global logger; it forwards each
/// record to whatever host [`Logger`] is currently installed in [`sink`].
struct LogBridge;

/// The Rust crates whose `debug` records are worth a support session.
///
/// Everything this product ships, plus the engine underneath it. Matched as a prefix, so
/// `mailcal_app::sync_account` and `provider_imap::watch` are both in.
const OUR_TARGETS: [&str; 5] = ["mailcal", "engine_", "provider_", "store_sqlite", "dav_"];

/// Whether `target` is one of ours.
fn is_ours(target: &str) -> bool {
    OUR_TARGETS.iter().any(|prefix| target.starts_with(prefix))
}

impl log::Log for LogBridge {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // The macros already gate on the global max level; this guards direct `log` calls.
        if metadata.level() > log::max_level() {
            return false;
        }
        // **`debug` is ours alone.** Turning it on is a support action; "show me what the app
        // did", and dependencies answer that question with their own internals. The HTML parser
        // emits a record per text node it parses, so a sync over a real mailbox fills the log's
        // rotating cap with parser state and evicts the very lines the session was enabled to
        // capture. It also costs real time: every record is formatted and crossed to the host.
        //
        // Nothing is filtered below `debug`. A dependency's `warn` is exactly what a support log
        // should keep; it is the level at which someone else's crate says something went wrong.
        metadata.level() <= log::Level::Info || is_ours(metadata.target())
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // A native fault is writing its own record straight to the file, on a descriptor no lock
        // here can reach (`crate::native_fault`). Standing down keeps this line out of the middle
        // of that stack, and lasts only as long as the record does. This covers the core's own
        // stream on every platform: a host that logs on its own account during a fault is the
        // client's to stand down, as Apple's `FileLog` does.
        if crate::native_fault::is_writing_fault_record() {
            return;
        }
        if let Some(logger) = sink().read().expect("log sink poisoned").as_ref() {
            logger.log(
                record.level().into(),
                record.target().to_owned(),
                record.args().to_string(),
            );
        }
    }

    fn flush(&self) {}
}

/// Installs `logger` as the sink for every `log` record and sets the global ceiling to
/// `level`. Idempotent across constructors: the first call registers the global
/// [`LogBridge`]; every call (re)points the sink and updates the level. Called before any
/// work so boot diagnostics are captured from the first line, which is also why the panic
/// hook ([`crate::crash`]) is armed from here.
pub(crate) fn install_logger(logger: Box<dyn Logger>, level: LogLevel) {
    *sink().write().expect("log sink poisoned") = Some(logger);
    // `set_boxed_logger` succeeds once per process; on a later constructor it returns Err
    // and we keep the already-registered bridge (the sink swap above redirected output).
    let _ = log::set_boxed_logger(Box::new(LogBridge));
    log::set_max_level(level.into());
    // The sink is pointed, so a panic from here on has somewhere to land.
    crate::crash::install_panic_hook();
}

/// Updates the global log ceiling at runtime (a support session toggling on `debug`),
/// without reconnecting anything. See [`MailcalApp::set_log_level`](crate::MailcalApp).
pub(crate) fn set_level(level: LogLevel) {
    log::set_max_level(level.into());
}

#[cfg(test)]
mod tests {
    use super::is_ours;

    #[test]
    fn debug_belongs_to_our_crates_and_the_engine_under_them() {
        assert!(is_ours("mailcal_app::sync_account"));
        assert!(is_ours("mailcal_bindings::boot"));
        assert!(is_ours("engine_sync::threading"));
        assert!(is_ours("provider_imap::watch"));
        assert!(is_ours("store_sqlite::migrations"));
    }

    #[test]
    fn a_dependency_that_narrates_its_own_internals_is_not() {
        // The HTML parser emits a record per text node; at debug that fills the log's rotating
        // cap during one sync and evicts what the session was enabled to capture.
        assert!(!is_ours("html5ever::tree_builder"));
        assert!(!is_ours("rustls::client::hs"));
        assert!(!is_ours("hyper_util::client::legacy"));
    }
}
