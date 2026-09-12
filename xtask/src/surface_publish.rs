//! A published surface may only be signalled by publishing it.
//!
//! `Surfaced<T>` (crates/mailcal-app/src/surfaced.rs) makes the write and the signal one operation,
//! so a host that pulls the instant it is told can never read the previous snapshot. The type
//! closes the door for anyone who uses it, but `App` still owns an `observer`, and
//! `self.observer.surface_changed(Surface::MailboxList)` would compile fine and silently reopen the
//! bug, in a form no test would catch: a stale paint is not a panic, and there is no second signal
//! coming to correct it.
//!
//! So this refuses two things across the crate:
//!
//! 1. Signalling a published surface anywhere but `surfaced.rs`. Announce it by publishing a value,
//!    or, when what went stale is not this snapshot, by `resignal()`, which cannot run ahead of a
//!    write because it performs none.
//! 2. Declaring one of those snapshot fields as a bare `Mutex`, which would take it back out of the
//!    type's hands entirely.
//!
//! Adding a surface to [`PUBLISHED`] is the deliberate act of putting it under the rule; a surface
//! whose pull recomputes from live state (`Settings`, `Connectivity`) has no stored snapshot and
//! does not belong here.

use std::path::Path;

use crate::git;

/// The crate the rule is about.
const CRATE: &str = "crates/mailcal-app/src";

/// The one file allowed to raise these signals, because it is the type that owns them.
const HOME: &str = "crates/mailcal-app/src/surfaced.rs";

/// Each published surface and the `App` field holding its snapshot.
const PUBLISHED: &[(&str, &str)] = &[
    ("MailboxList", "mailbox_list"),
    ("Calendar", "calendar"),
    ("Reading", "reading"),
    ("Sending", "send_status"),
];

/// Runs the check. `Ok(true)` means no published surface is signalled behind the type's back.
///
/// # Errors
///
/// Fails when git cannot list the crate's sources, and when it lists none: a check with nothing to
/// read would pass on a tree that had moved the crate out from under it.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let files: Vec<String> = git::listed(root, &[&format!("{CRATE}/*.rs")])?;
    if files.is_empty() {
        return Err(format!(
            "found no sources under {CRATE}: is the path still right?"
        ));
    }

    let mut problems: Vec<String> = Vec::new();
    for name in &files {
        let Ok(text) = std::fs::read_to_string(root.join(name)) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            let number = number + 1;
            if name != HOME {
                problems.extend(stray_signal(name, number, line));
            }
            problems.extend(bare_field(name, number, line));
        }
    }

    if problems.is_empty() {
        println!(
            "OK: {} published surface(s) are only signalled by publishing, across {} file(s).",
            PUBLISHED.len(),
            files.len()
        );
        return Ok(true);
    }

    eprintln!("Published surfaces must be announced by publishing them:\n");
    for problem in &problems {
        eprintln!("  {problem}\n");
    }
    eprintln!("See {HOME} for why.");
    Ok(false)
}

/// A direct `surface_changed(Surface::X)` for a surface that has a snapshot.
fn stray_signal(name: &str, number: usize, line: &str) -> Option<String> {
    let surface = signalled_surface(line)?;
    let (_, field) = PUBLISHED.iter().find(|(s, _)| *s == surface)?;
    Some(format!(
        "{name}:{number}: Surface::{surface} is published, so it may not be signalled \
         directly.\n    Use self.{field}.publish(value), or self.{field}.resignal() if there is no \
         new value.\n    {}",
        line.trim()
    ))
}

/// The surface named by a `surface_changed(Surface::X)` call on this line.
///
/// `crate::` is optional in front of the enum, and whitespace is allowed around the argument, which
/// is what the original's pattern allowed for a call rustfmt had wrapped.
fn signalled_surface(line: &str) -> Option<&str> {
    let after = &line[line.find("surface_changed")? + "surface_changed".len()..];
    let inner = after.trim_start().strip_prefix('(')?.trim_start();
    let inner = inner.strip_prefix("crate::").unwrap_or(inner);
    let name = inner.strip_prefix("Surface::")?;
    let end = name
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(name.len());
    let (name, rest) = name.split_at(end);
    rest.trim_start().starts_with(')').then_some(name)
}

/// A snapshot field declared as a plain `Mutex` instead of a `Surfaced`.
fn bare_field(name: &str, number: usize, line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let (surface, field) = PUBLISHED.iter().find(|(_, field)| {
        trimmed
            .strip_prefix(*field)
            .and_then(|rest| rest.strip_prefix(':'))
            .is_some_and(|rest| rest.trim_start().starts_with("Mutex<"))
    })?;
    Some(format!(
        "{name}:{number}: `{field}` backs Surface::{surface} and must stay a `Surfaced<_>`; a bare \
         Mutex separates the write from the signal again.\n    {}",
        line.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::{bare_field, signalled_surface, stray_signal};

    #[test]
    fn reads_the_surface_a_call_signals() {
        assert_eq!(
            signalled_surface("self.observer.surface_changed(Surface::Reading);"),
            Some("Reading")
        );
        // rustfmt wraps a long call, and the enum may be fully qualified.
        assert_eq!(
            signalled_surface("    .surface_changed( crate::Surface::Calendar )"),
            Some("Calendar")
        );
    }

    #[test]
    fn something_that_is_not_a_signal_is_not_read_as_one() {
        assert_eq!(
            signalled_surface("fn surface_changed(&self, s: Surface)"),
            None
        );
        assert_eq!(
            signalled_surface("// surface_changed is the port method"),
            None
        );
    }

    #[test]
    fn a_surface_with_no_snapshot_is_not_under_the_rule() {
        // `Settings` recomputes on pull, so signalling it directly is how it is meant to work.
        let line = "self.observer.surface_changed(Surface::Settings);";
        assert_eq!(signalled_surface(line), Some("Settings"));
        assert!(stray_signal("crates/mailcal-app/src/app.rs", 1, line).is_none());
    }

    #[test]
    fn a_published_surface_signalled_directly_is_caught() {
        let line = "        self.observer.surface_changed(Surface::MailboxList);";
        let problem = stray_signal("crates/mailcal-app/src/app.rs", 12, line).unwrap();
        assert!(problem.contains("Surface::MailboxList is published"));
        assert!(problem.contains("self.mailbox_list.publish(value)"));
    }

    #[test]
    fn a_bare_mutex_behind_a_published_surface_is_caught() {
        let line = "    mailbox_list: Mutex<MailboxListSnapshot>,";
        assert!(bare_field("crates/mailcal-app/src/app.rs", 30, line).is_some());
    }

    #[test]
    fn the_type_that_owns_the_snapshot_is_fine() {
        let line = "    mailbox_list: Surfaced<MailboxListSnapshot>,";
        assert!(bare_field("crates/mailcal-app/src/app.rs", 30, line).is_none());
        // And a field of another name that happens to hold a Mutex is nobody's business.
        let other = "    pending: Mutex<Vec<Job>>,";
        assert!(bare_field("crates/mailcal-app/src/app.rs", 31, other).is_none());
    }
}
