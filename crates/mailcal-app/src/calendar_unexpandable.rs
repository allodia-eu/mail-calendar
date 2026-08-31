//! What an event the expander refused writes to the diagnostic log, as a pure function.
//!
//! The line lives here rather than inline in [`crate::calendar_ops`] for the reason
//! `mailcal_bindings::credential_log` gives: the privacy decision in a log line is one careless
//! edit from leaking, and a function returning a `String` can be asserted on without a logger.
//!
//! # Why the line exists
//!
//! An event whose recurrence the engine cannot expand is stored and materializes **zero**
//! occurrences, so it is invisible to every range read and the grid draws it nowhere. It does
//! not look wrong; it is absent, with nothing anywhere saying why. The engine carries the
//! refusals out for exactly this reason; discarding them at the call site is the failure the
//! report was added to prevent.
//!
//! # Why the event's key is not in it
//!
//! Counts and reasons only. The engine identifies each refusal by its `ProviderKey`, and on
//! CalDAV that key is the resource href (routinely `/dav/cal/<address>/…`) so logging it
//! would write the user's own email address into the file they attach to a support request
//! ([`docs/logging.md`](../../../docs/logging.md), the same trap that keeps connect steps from
//! carrying their URL). The reason is engine-authored and names a rule part, never the event.

use std::collections::BTreeMap;

use engine_api::UnexpandableEvent;

/// One line summarising the events the engine could not expand, or `None` when it expanded
/// everything.
///
/// Reasons are tallied rather than listed per event: a series that hits an unsupported rule
/// usually arrives alongside its siblings, and twenty identical lines bury the rest of the
/// pass in a size-capped log.
pub(crate) fn unexpandable_line(refused: &[UnexpandableEvent]) -> Option<String> {
    if refused.is_empty() {
        return None;
    }
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    for event in refused {
        *tally.entry(event.reason.as_str()).or_default() += 1;
    }
    let reasons = tally
        .iter()
        .map(|(reason, count)| format!("{reason} x{count}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "{} event(s) cannot be shown: {reasons}",
        refused.len()
    ))
}

#[cfg(test)]
mod tests {
    use engine_api::ProviderKey;

    use super::*;

    fn refused(key: &str, reason: &str) -> UnexpandableEvent {
        UnexpandableEvent {
            event: ProviderKey::new(key).expect("a provider key"),
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn an_account_that_expanded_everything_writes_nothing() {
        // A line that appears on every pass is a line people stop reading.
        assert_eq!(unexpandable_line(&[]), None);
    }

    #[test]
    fn the_line_counts_the_events_and_names_the_reasons() {
        let line = unexpandable_line(&[
            refused("a", "unsupported recurrence rule: BYSETPOS"),
            refused("b", "unsupported recurrence rule: BYSETPOS"),
            refused(
                "c",
                "unsupported recurrence rule: RSCALE / non-Gregorian recurrence",
            ),
        ])
        .expect("three refusals produce a line");
        assert!(line.starts_with("3 event(s) cannot be shown: "), "{line}");
        assert!(line.contains("BYSETPOS x2"), "{line}");
        assert!(
            line.contains("RSCALE / non-Gregorian recurrence x1"),
            "{line}"
        );
    }

    #[test]
    fn the_line_never_carries_the_event_key() {
        // The rule with teeth. A CalDAV key is the resource href, which carries the user's own
        // address: so this asserts on a key shaped like the real thing, not an opaque id.
        let href = "/dav/cal/someone%40example.com/default/quarterly.ics";
        let line = unexpandable_line(&[refused(href, "unsupported recurrence rule: BYSETPOS")])
            .expect("one refusal produces a line");
        assert!(!line.contains("example.com"), "{line}");
        assert!(!line.contains("someone"), "{line}");
        assert!(!line.contains("quarterly"), "{line}");
        assert!(!line.contains("/dav/"), "{line}");
    }
}
