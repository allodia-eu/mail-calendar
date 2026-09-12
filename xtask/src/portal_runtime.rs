//! Guards the single Tokio runtime the Linux client's desktop-portal callers share.
//!
//! `ashpd` caches one `zbus::Connection` for the whole process, and zbus drives that connection
//! from whichever runtime opened it. A runtime built per call, or owned by one service, takes the
//! connection's reader with it when it is dropped while the connection itself stays cached. Every
//! later portal call then awaits a reply that can never arrive: no error, no timeout, a thread
//! parked for the life of the process, and whatever state machine that thread was serving parked
//! with it.
//!
//! It fails in the worst possible way, because **the first portal call of the process succeeds**. A
//! manual check passes, a screenshot proves nothing, and only the second call hangs; and the two
//! callers are far apart, so the store that seeds the connection and the notification that hangs on
//! it look like unrelated code. That is why the shape is caught in the source rather than in a run.
//!
//! Test code may build its own: it reaches no portal, and the secure store's nesting guard needs
//! two distinct runtimes to prove anything at all. A file's first `#[cfg(test)]` marks where that
//! begins, and a `*_tests.rs` file is test code throughout.

use std::path::Path;

use crate::git::{self, Kind};

/// The one module allowed to build a runtime, because it owns the only one and never drops it.
const OWNER: &str = "clients/linux/src/host_runtime.rs";

/// The ways a Tokio runtime is constructed.
const BUILDERS: &str = "runtime::Builder|Builder::new_multi_thread|Builder::new_current_thread";

/// Where test code begins in a file that is otherwise production code.
const TEST_MARKER: &str = "#[cfg(test)]";

/// Runs the check. `Ok(true)` means every portal caller takes the shared runtime.
///
/// # Errors
///
/// Propagates a git failure: an error must never read as "no matches", because a check that
/// reports OK because it could not look is worse than no check.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let hits = git::grep(root, Kind::Extended, BUILDERS, &["clients/linux/**/*.rs"])?;

    let mut failed = false;
    for hit in &hits.lines {
        let Some((file, line, _)) = split_hit(hit) else {
            continue;
        };
        if file == OWNER || file.ends_with("_tests.rs") {
            continue;
        }
        if is_below_the_test_marker(root, file, line) {
            continue;
        }
        println!("ERROR: {file} builds a Tokio runtime of its own.\n    {hit}");
        failed = true;
    }

    if failed {
        eprintln!(
            "
Take the shared one instead:

    let Some(runtime) = crate::host_runtime::shared() else {{ ... }};

It outlives every caller, so the portal connection it opened stays driven. A runtime of your own
works exactly once and then hangs the process for good (docs/client-traps.md)."
        );
        return Ok(false);
    }

    println!("OK: every Linux portal caller takes the one shared Tokio runtime.");
    Ok(true)
}

/// True when `line` falls after the file's first `#[cfg(test)]`.
///
/// A file that cannot be read answers false, which reports the hit rather than excusing it. The
/// file came out of a search over this tree, so being unable to read it is a surprise, and the safe
/// direction for a surprise is the one that shows a human the line.
fn is_below_the_test_marker(root: &Path, file: &str, line: usize) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join(file)) else {
        return false;
    };
    text.lines()
        .position(|l| l.contains(TEST_MARKER))
        .is_some_and(|index| line > index + 1)
}

/// Splits a `path:line:content` hit.
fn split_hit(hit: &str) -> Option<(&str, usize, &str)> {
    let (path, rest) = hit.split_once(':')?;
    let (number, content) = rest.split_once(':')?;
    Some((path, number.parse().ok()?, content))
}

#[cfg(test)]
mod tests {
    use super::split_hit;

    #[test]
    fn splits_a_hit_with_colons_in_the_content() {
        let (path, line, content) =
            split_hit("a/b.rs:12:let rt = runtime::Builder::new();").unwrap();
        assert_eq!((path, line), ("a/b.rs", 12));
        assert_eq!(content, "let rt = runtime::Builder::new();");
    }

    #[test]
    fn a_line_without_a_number_is_not_a_hit() {
        assert!(split_hit("a/b.rs:not-a-number:text").is_none());
    }
}
