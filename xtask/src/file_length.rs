//! Fails when a tracked source file exceeds the line ceiling.
//!
//! AGENTS.md product hard rule: "Files stay under 500 lines; split by responsibility." The rule is
//! not language-qualified, so neither is this check: it is the machine enforcement for every
//! language whose formatter or linter has no per-file length lint (rustfmt and clippy have none;
//! neither does `dotnet format`).
//!
//! To bring another language under the rule, add its extension to `PATTERNS` and split whatever the
//! run then flags.
//!
//! A file may be exempted only for being a committed build output (see `EXCLUDED`). "It is hard to
//! split" is not a reason: that is the rule working.
//!
//! This is the check the port was worth doing for on its own. The shell original started `wc` once
//! per file, and on an arm64 Windows host, where every process is an emulated x86_64 Cygwin fork,
//! 900 files at roughly 200ms apiece came to three minutes. Reading them here costs a tenth of a
//! second.

use std::path::Path;

/// The ceiling, in newlines.
const MAX: usize = 500;

/// The extensions under the rule. Every tracked file matching one of these is checked.
const PATTERNS: &[&str] = &["*.rs", "*.cs", "*.ts", "*.js", "*.html", "*.swift", "*.kt"];

/// The one exception, and the only kind there can be: a build output that is committed rather than
/// generated per build. `clients/composer/dist/editor.html` is the whole rich editor inlined into a
/// single self-contained file, because that is what its four WebView hosts can load (see the file's
/// own header). Its sources, `clients/composer/src/*.ts` and that directory's `index.html`, are
/// tracked, checked here like everything else, and are where the rule actually bites.
const EXCLUDED: &[&str] = &["clients/composer/dist/editor.html"];

/// Runs the check. `Ok(true)` means every file is within the ceiling.
///
/// # Errors
///
/// Propagates a git failure: an error that read as "no files" would be a check that cannot fail.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut over: Vec<(String, usize)> = Vec::new();

    for name in crate::git::ls_files(root, PATTERNS)? {
        if EXCLUDED.contains(&name.as_str()) {
            continue;
        }
        // A tracked file that is deleted but not staged yet still appears in `git ls-files`. The
        // pipeline never sees that state, but a working tree preparing a deletion does.
        if let Ok(bytes) = std::fs::read(root.join(&name)) {
            let lines = count_lines(&bytes);
            if lines > MAX {
                over.push((name, lines));
            }
        }
    }

    if over.is_empty() {
        println!(
            "OK: every tracked {} file is within the {MAX}-line limit.",
            PATTERNS.join(" ")
        );
        return Ok(true);
    }

    for (name, lines) in &over {
        println!("  {name}: {lines} lines");
    }
    eprintln!(
        "ERROR: the file(s) above exceed the {MAX}-line limit: split them by responsibility."
    );
    Ok(false)
}

/// Newline count, matching `wc -l`.
///
/// A final line with no trailing newline is not counted. That is deliberate: the ceiling is a rule
/// of thumb, and matching the tool developers reach for keeps the number this prints the same as
/// the one they see.
// `naive_bytecount` wants the `bytecount` crate. A whole dependency to shave microseconds off a
// check that already runs in a tenth of a second is the wrong trade: this crate's value is that it
// compiles from nothing, and every crate added here is one more thing to build before an instant
// check can run.
#[allow(
    clippy::naive_bytecount,
    reason = "a dependency would cost more than it saves"
)]
fn count_lines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|b| **b == b'\n').count()
}

#[cfg(test)]
mod tests {
    use super::count_lines;

    #[test]
    fn counts_newlines_not_lines() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one\n"), 1);
        assert_eq!(count_lines(b"one\ntwo\n"), 2);
    }

    #[test]
    fn a_final_line_without_a_newline_is_not_counted() {
        // What `wc -l` does, and therefore what the ceiling has always meant.
        assert_eq!(count_lines(b"one\ntwo"), 1);
    }

    #[test]
    fn carriage_returns_do_not_add_a_line() {
        // A CRLF checkout must not read as twice the length.
        assert_eq!(count_lines(b"one\r\ntwo\r\n"), 2);
    }
}
