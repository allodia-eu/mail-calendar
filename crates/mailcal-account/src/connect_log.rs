//! The connect phase's diagnostic trace: an [`engine_provider::ConnectObserver`] that writes each
//! step of a provider connect: the well-known redirect hops, the TLS handshake, authentication,
//! endpoint discovery; to the shared diagnostic log.
//!
//! Before this, `Provider::connect` was a black box: a failed connect logged one error, with no way
//! to tell *where* it broke: a redirect chain that walked off to the wrong origin, TLS that never
//! came up, or credentials the server refused. Each is a different support answer, and the steps
//! separate them.
//!
//! **What is logged in full: the negotiated dialect.** `ConnectStep::Negotiated` carries a protocol
//! dialect and its capability atoms; facts about the *server's software*, not about the account;
//! so none of the forbidden categories apply and the payload goes into the line verbatim. It earns
//! its place: two accounts on one build behave differently because their servers agreed to
//! different things, and this is the only line that says which.
//!
//! **What is deliberately not logged: the URLs.** [`docs/logging.md`](../../../docs/logging.md)
//! forbids an account id, address, username, **host, endpoint**, or credential in the log, and the
//! step payloads carry exactly those: a JMAP `apiUrl`, a CalDAV calendar-home href. The engine
//! scrubs `userinfo` (`user:pass@host`) from every URL it hands an observer, but that is *not*
//! enough for this rule: a CalDAV home href is routinely `/calendars/<address>/`, and the scrub
//! leaves an `@` outside the authority untouched (by design: it may be a path). Logging the
//! endpoint would therefore write the user's own address into a file meant to be safe to attach to
//! a support request. So each step is recorded as the **event** it is, and its URL is dropped;
//! which is what a diagnosis needs anyway ("TLS came up, auth was refused"), and keeps the log
//! attachable.

use std::sync::Arc;

use engine_provider::{ConnectObserver, ConnectStep};

/// An observer that records `protocol`'s connect steps in the diagnostic log: the URL-free trace
/// described in this module's docs.
///
/// `protocol` (`imap` / `jmap` / `caldav`) labels the line, because several accounts connect
/// concurrently at boot and their steps interleave in one log.
///
/// Carried on the provider's *config*, so every connect built from it is traced; including an
/// `ImapWatcher`'s separate `IDLE` connection and a `ReconnectingImapProvider`'s re-dial, which is
/// precisely when a connect problem shows up in the field.
pub(crate) fn connect_logger(protocol: &'static str) -> Arc<dyn ConnectObserver> {
    Arc::new(move |step: &ConnectStep<'_>| {
        if let Some(line) = step_line(protocol, step) {
            log::info!("{line}");
        }
    })
}

/// The log line for one step: the whole privacy decision, in one pure function so it can be
/// tested directly (see this module's tests: no step's URL may survive into the line).
fn step_line(protocol: &str, step: &ConnectStep<'_>) -> Option<String> {
    let event = match step {
        // The hop's URL is the endpoint the rule forbids, that it redirected at all is the
        // diagnostic ("we did not end up talking to the server you configured").
        ConnectStep::Redirected { .. } => "followed a redirect".to_owned(),
        ConnectStep::TlsEstablished(version) => format!("TLS established ({version:?})"),
        ConnectStep::Authenticated => "authenticated".to_owned(),
        ConnectStep::Discovered { .. } => "endpoint discovered".to_owned(),
        // The one step whose payload is *safe* to log in full: both halves name the server's
        // software (a protocol dialect and its capability atoms) and neither is an account id,
        // address, host or endpoint. It is also the most useful line here, because it is what
        // separates two accounts that behave differently on the same build.
        ConnectStep::Negotiated {
            dialect, features, ..
        } => {
            if features.is_empty() {
                format!("{dialect}, no optional extensions")
            } else {
                format!("{dialect}, extensions: {}", features.join(", "))
            }
        }
        // `ConnectStep` is `#[non_exhaustive]`: a step the engine adds later must not be logged
        // unreviewed; its payload has not been checked against the privacy rule above.
        _ => return None,
    };
    Some(format!("connect[{protocol}]: {event}"))
}

#[cfg(test)]
mod tests {
    use engine_provider::TlsVersion;

    use super::step_line;
    use crate::connect_log::ConnectStep;

    /// The steps that carry a URL must never put it in the log: `docs/logging.md` forbids a host,
    /// an endpoint, or an address there. The CalDAV case is the trap; its calendar-home href
    /// routinely *is* the user's address, and the engine's scrub only strips `userinfo`, so an `@`
    /// in the path survives all the way to the observer.
    #[test]
    fn a_steps_url_never_reaches_the_log() {
        let home = "https://dav.example.com/calendars/alice@example.com/";
        let discovered = step_line("caldav", &ConnectStep::discovered(home)).unwrap();
        let redirected = step_line(
            "jmap",
            &ConnectStep::redirected("https://example.com/.well-known/jmap", home),
        )
        .unwrap();

        for line in [&discovered, &redirected] {
            assert!(
                !line.contains('@') && !line.contains("example.com") && !line.contains("http"),
                "a connect step leaked its URL into the log: {line}",
            );
        }
        assert_eq!(discovered, "connect[caldav]: endpoint discovered");
        assert_eq!(redirected, "connect[jmap]: followed a redirect");
    }

    /// The negotiated dialect goes into the line whole; it names the server's software, not the
    /// account, and reports what the session may *use*, so a rev2 account shows the extensions
    /// the dialect folded in even though its server advertised none of them separately.
    #[test]
    fn the_negotiated_dialect_and_its_extensions_are_logged_in_full() {
        assert_eq!(
            step_line(
                "imap",
                &ConnectStep::negotiated("IMAP4rev2", &["IDLE", "LIST-STATUS", "SPECIAL-USE"])
            )
            .unwrap(),
            "connect[imap]: IMAP4rev2, extensions: IDLE, LIST-STATUS, SPECIAL-USE",
        );
        // A bare server still says which dialect it settled on, which is the half that matters.
        assert_eq!(
            step_line("imap", &ConnectStep::negotiated("IMAP4rev1", &[])).unwrap(),
            "connect[imap]: IMAP4rev1, no optional extensions",
        );
    }

    /// The URL-free steps are what a support diagnosis actually reads: TLS came up (and at which
    /// version), and the server accepted the credentials.
    #[test]
    fn the_url_free_steps_are_logged_in_full() {
        assert_eq!(
            step_line("imap", &ConnectStep::TlsEstablished(TlsVersion::Tls1_3)).unwrap(),
            "connect[imap]: TLS established (Tls1_3)",
        );
        assert_eq!(
            step_line("imap", &ConnectStep::Authenticated).unwrap(),
            "connect[imap]: authenticated",
        );
    }
}
