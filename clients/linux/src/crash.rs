//! GTK's own diagnostics, in the log the user can actually hand over.
//!
//! The core's panic hook (`crates/mailcal-bindings/src/crash.rs`) already covers this client for
//! free, because the client *is* a Rust process; but GTK death rarely arrives as a Rust panic.
//! A failed precondition inside GTK or libadwaita raises a GLib **critical** and carries on; a
//! `g_error` aborts the process outright. Neither is a Rust panic, and by default both go to
//! stderr and the journal; nowhere near the file Settings → Diagnostics offers to share. A user's
//! support log therefore said nothing about the thing that broke.
//!
//! Warnings and above are forwarded; a GLib `message`, `info` or `debug` record is left to the
//! default handler alone, for the reason `logging.rs` gives for filtering dependency `debug`: a
//! toolkit narrating its own internals fills the rotating cap and evicts what the session was
//! opened to capture.

use gtk::glib::{self, LogLevel};

/// Routes GLib's warnings, criticals and fatal errors into the app's log, then lets the default
/// handler run so stderr and the journal are unchanged.
///
/// Installed once the core is up, because that is when the `log` facade has a sink: a record
/// raised before then has nowhere to land. What that leaves uncovered is GTK's own start-up, and
/// [`crate::boot`] is where the reasoning for accepting it lives.
pub(crate) fn capture_toolkit_diagnostics() {
    glib::log_set_default_handler(|domain, level, message| {
        if let Some((severity, text)) = record(domain, level, message) {
            log::log!(severity, "{text}");
        }
        glib::log_default_handler(domain, level, Some(message));
    });
}

/// The line one GLib record writes, or `None` when it is not one we forward.
///
/// A critical is not a crash, so it does **not** carry `unhandled`: that word is the one string
/// support greps across four clients for "the process died", and spending it on a recoverable
/// precondition failure would make every search useless. `G_LOG_LEVEL_ERROR` is the exception:
/// GLib aborts as soon as the handler returns, so that record really is the app's last words.
fn record(domain: Option<&str>, level: LogLevel, message: &str) -> Option<(log::Level, String)> {
    let source = domain.unwrap_or("GTK");
    match level {
        LogLevel::Error => Some((
            log::Level::Error,
            format!("unhandled error from {source}, the app is stopping: {message}"),
        )),
        LogLevel::Critical => Some((
            log::Level::Error,
            format!("critical from {source}: {message}"),
        )),
        LogLevel::Warning => Some((
            log::Level::Warn,
            format!("warning from {source}: {message}"),
        )),
        LogLevel::Message | LogLevel::Info | LogLevel::Debug => None,
    }
}

#[cfg(test)]
mod tests {
    use gtk::glib::LogLevel;

    use super::record;

    #[test]
    fn only_the_fatal_record_spends_the_word_support_greps() {
        let (level, text) = record(Some("Gtk"), LogLevel::Error, "boom").expect("forwarded");
        assert_eq!(level, log::Level::Error);
        assert!(
            text.starts_with("unhandled error from Gtk, the app is stopping: "),
            "{text}"
        );

        for recoverable in [LogLevel::Critical, LogLevel::Warning] {
            let (_, text) = record(Some("Gtk"), recoverable, "boom").expect("forwarded");
            assert!(
                !text.contains("unhandled"),
                "a recoverable record must not read as a crash: {text}"
            );
        }
    }

    #[test]
    fn a_critical_is_an_error_and_a_warning_is_a_warning() {
        // A failed precondition inside GTK is the shape that precedes a visible defect, so it has
        // to survive the INFO default the log ships at.
        assert_eq!(
            record(Some("Adwaita"), LogLevel::Critical, "boom")
                .expect("forwarded")
                .0,
            log::Level::Error
        );
        assert_eq!(
            record(Some("Adwaita"), LogLevel::Warning, "boom")
                .expect("forwarded")
                .0,
            log::Level::Warn
        );
    }

    #[test]
    fn the_toolkits_own_narration_is_left_to_stderr() {
        for chatter in [LogLevel::Message, LogLevel::Info, LogLevel::Debug] {
            assert!(record(Some("Gtk"), chatter, "some detail").is_none());
        }
    }

    #[test]
    fn a_record_with_no_domain_still_names_where_it_came_from() {
        let (_, text) = record(None, LogLevel::Critical, "boom").expect("forwarded");
        assert_eq!(text, "critical from GTK: boom");
    }

    #[test]
    fn the_message_survives_verbatim() {
        // It is the only half a reader can act on; a truncated or reworded GTK message sends
        // whoever reads the log looking for a string the toolkit never printed.
        let raw = "Failed to set text 'Wire transfer & invoice' from markup";
        for level in [LogLevel::Error, LogLevel::Critical, LogLevel::Warning] {
            let (_, text) = record(Some("Gtk"), level, raw).expect("forwarded");
            assert!(text.ends_with(raw), "{text}");
        }
    }
}
