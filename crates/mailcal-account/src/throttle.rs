//! The host half of throttling: one policy every provider gets, and the log line that
//! explains a pause a user would otherwise just experience.
//!
//! The waiting itself is the engine's, in one place for every HTTP provider
//! (`engine-http`). What cannot be the engine's is the log: it writes none, by design, so a
//! throttle reaches a diagnostic log only through an observer a host implements. Without one,
//! a mail server slowing us down looks exactly like the app being slow.

use std::sync::{Arc, OnceLock};

use engine_api::{RetryConfig, ThrottleEvent, ThrottleObserver};

/// The throttling policy every provider this account connects is given: the sibling of
/// [`account_tls`](crate::tls::account_tls).
///
/// Built once: it is the same for every account, and the observer behind it is stateless.
pub(crate) fn account_retry() -> RetryConfig {
    static RETRY: OnceLock<RetryConfig> = OnceLock::new();
    RETRY
        .get_or_init(|| RetryConfig::default().with_observer(Arc::new(LogThrottles)))
        .clone()
}

/// Writes every throttle to the app log.
struct LogThrottles;

impl ThrottleObserver for LogThrottles {
    fn throttled(&self, event: &ThrottleEvent<'_>) {
        let (level, line) = describe(event);
        log::log!(level, "{line}");
    }
}

/// The log record one event becomes.
///
/// Separated from writing it so the wording and the level are asserted directly; installing a
/// logger to read them back would be a process-global the rest of the suite shares.
fn describe(event: &ThrottleEvent<'_>) -> (log::Level, String) {
    let provider = event.provider;
    let millis = event.delay.as_millis();
    if event.gave_up {
        // The one a slow sync is explained by, so it is a warning: the pass stopped early
        // and the rest waits for the next one.
        return (
            log::Level::Warn,
            format!(
                "{provider}: still limiting us after {} tries and {millis}ms of waiting; \
                 the rest waits for the next sync",
                event.attempt.saturating_add(1),
            ),
        );
    }
    let next = event.attempt.saturating_add(2);
    let line = if event.server_asked {
        format!(
            "{provider}: limiting how fast we can fetch; it asked for {millis}ms before try {next}"
        )
    } else {
        format!("{provider}: limiting how fast we can fetch; waiting {millis}ms before try {next}")
    };
    (log::Level::Info, line)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use engine_api::ThrottleEvent;

    use super::describe;

    fn event() -> ThrottleEvent<'static> {
        ThrottleEvent {
            provider: "gmail",
            status: 429,
            attempt: 0,
            delay: Duration::from_millis(750),
            server_asked: false,
            gave_up: false,
        }
    }

    #[test]
    fn an_absorbed_wait_is_reported_without_alarming_anyone() {
        let (level, line) = describe(&event());
        assert_eq!(level, log::Level::Info);
        assert!(line.contains("gmail"), "{line}");
        assert!(line.contains("750ms"), "{line}");
        assert!(line.contains("try 2"), "{line}");
    }

    #[test]
    fn a_server_that_named_its_own_delay_says_so() {
        let (_, line) = describe(&ThrottleEvent {
            server_asked: true,
            delay: Duration::from_secs(30),
            ..event()
        });
        assert!(line.contains("it asked for 30000ms"), "{line}");
    }

    #[test]
    fn giving_up_is_a_warning_and_says_the_work_is_not_lost() {
        let (level, line) = describe(&ThrottleEvent {
            gave_up: true,
            attempt: 4,
            ..event()
        });
        assert_eq!(level, log::Level::Warn);
        assert!(line.contains("5 tries"), "{line}");
        assert!(line.contains("next sync"), "{line}");
    }

    #[test]
    fn no_line_can_name_the_users_mail() {
        // The event carries no URL by construction; this locks the log line to the same rule
        // (`docs/logging.md`), since a request path names a mailbox or a message.
        for gave_up in [false, true] {
            for server_asked in [false, true] {
                let (_, line) = describe(&ThrottleEvent {
                    gave_up,
                    server_asked,
                    ..event()
                });
                assert!(!line.contains("http"), "{line}");
                assert!(!line.contains('@'), "{line}");
            }
        }
    }
}
