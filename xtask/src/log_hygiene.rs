//! Fails when a diagnostic log line names this repository instead of the user's mail.
//!
//! The log is a file the user opens and attaches to a support request, so every line is product
//! surface (`docs/logging.md` → "A log line describes the user's mail, never our source tree").
//! This checks only the part of that rule a machine can decide without guessing:
//!
//! * a path inside this repository, or any `.md` reference;
//! * an issue or pull-request number;
//! * a raw account-id interpolation.
//!
//! Deliberately **not** checked: internal jargon ("the registry", a type name). That is the larger
//! half of the rule and it needs judgement; a checker that guesses at prose produces false
//! positives, and a checker people learn to skip protects nothing. The narrow half is worth having
//! because it is exact, and because it is the half that actually shipped: an `error!` citing a
//! design document by name reached a production build.
//!
//! Rust, Kotlin, Swift and C# are scanned for logging calls, and only their *string literals* are
//! inspected, so a comment above a log line, which is where our reasoning is supposed to live, is
//! never flagged.

use std::path::Path;

use crate::{git, prose};

/// Where the rule looks.
const ROOTS: &[&str] = &["crates", "clients"];

/// The source types a logging call can be written in.
const SOURCES: &[&str] = &["rs", "kt", "swift", "cs"];

/// How each language spells a logging call. Matched at the name, then the `(` that follows it.
const CALLS: &[&str] = &[
    // Rust, the `log` crate.
    "log::error!",
    "log::warn!",
    "log::info!",
    "log::debug!",
    "log::trace!",
    // Kotlin, Swift and C# instance loggers.
    "logger.error",
    "logger.warn",
    "logger.info",
    "logger.debug",
    // The C# static helper.
    "Log.Error",
    "Log.Warn",
    "Log.Info",
    "Log.Debug",
    // Apple.
    "NSLog",
    "os_log",
];

/// How far past a call the argument text is read, when nothing ends it sooner.
const ARGUMENT_WINDOW: usize = 4000;

/// One logged string that names this tree.
#[derive(Debug)]
struct Hit {
    file: String,
    line: usize,
    reason: &'static str,
    snippet: String,
}

/// Runs the check. `Ok(true)` means no log line names this repository.
///
/// # Errors
///
/// Propagates a git failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut checked = 0usize;
    let mut found: Vec<Hit> = Vec::new();

    for name in git::listed(root, ROOTS)? {
        if !SOURCES.contains(&prose::extension(&name)) {
            continue;
        }
        // `git ls-files` reads the index, so a source deleted in the working tree but not yet
        // staged is listed and cannot be opened. A checker that dies there fails the build for a
        // reason that has nothing to do with what it checks, which is how a gate gets disabled.
        let Ok(text) = std::fs::read_to_string(root.join(&name)) else {
            continue;
        };
        checked += 1;
        found.extend(hits(&name, &text));
    }

    if found.is_empty() {
        println!("OK: no log line in {checked} source file(s) names this repository.");
        return Ok(true);
    }

    for hit in &found {
        eprintln!(
            "{}:{}: a log line {}: '{}'",
            hit.file, hit.line, hit.reason, hit.snippet
        );
    }
    eprintln!(
        "\n{} log line(s) name this repository. The log is a file the user reads and attaches to a \
         support request, so it says what happened to their mail and what it means for them; our \
         reasoning goes in a comment beside the code, where a refactor keeps it true. See \
         docs/logging.md.",
        found.len()
    );
    Ok(false)
}

/// Every forbidden thing inside a logged string literal in one file.
fn hits(name: &str, text: &str) -> Vec<Hit> {
    let mut found = Vec::new();
    for (at, call) in calls(text) {
        let tail = &text[call..text.len().min(call + ARGUMENT_WINDOW)];
        let args = &tail[..args_end(tail).unwrap_or(tail.len())];
        for literal in string_literals(args) {
            for (reason, matched) in forbidden(literal) {
                found.push(Hit {
                    file: name.to_owned(),
                    line: text[..at].matches('\n').count() + 1,
                    reason,
                    snippet: matched,
                });
            }
        }
    }
    found
}

/// Every logging call in a file, as `(where the name starts, where its arguments start)`.
fn calls(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (at, _) in text.char_indices() {
        let rest = &text[at..];
        let Some(call) = CALLS.iter().find(|call| rest.starts_with(**call)) else {
            continue;
        };
        let after = &rest[call.len()..];
        let spaces = after
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(after.len());
        if after[spaces..].starts_with('(') {
            found.push((at, at + call.len() + spaces + 1));
        }
    }
    found
}

/// Where a call's argument text ends: the `)` closing the statement, or a blank line.
///
/// The blank line is the bound on a malformed call, so a checker reading past the end of one does
/// not wander into the next function and report it against the wrong line.
fn args_end(tail: &str) -> Option<usize> {
    for (at, _) in tail.match_indices('\n') {
        let rest = &tail[at + 1..];
        let spaces = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        if rest[..spaces].contains('\n') || rest[spaces..].starts_with(')') {
            return Some(at);
        }
    }
    None
}

/// Every string literal in a fragment of source, quotes included.
///
/// A backslash escapes whatever follows it, which is what keeps `\"` from closing the literal, and
/// a newline inside one is allowed: the wide log lines wrap their format strings.
fn string_literals(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'"' {
            at += 1;
            continue;
        }
        let mut end = at + 1;
        while end < bytes.len() {
            match bytes[end] {
                b'\\' => end += 2,
                b'"' => break,
                _ => end += 1,
            }
        }
        if end >= bytes.len() {
            break;
        }
        if let Some(literal) = text.get(at..=end) {
            found.push(literal);
        }
        at = end + 1;
    }
    found
}

/// What a logged string may never carry, and why.
fn forbidden(literal: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    if let Some(matched) = repository_path(literal) {
        found.push(("names a path in this repository", matched));
    }
    if let Some(matched) = document_reference(literal) {
        found.push(("cites a design document", matched));
    }
    if let Some(matched) = issue_number(literal) {
        found.push(("cites an issue or PR number", matched));
    }
    if let Some(matched) = account_interpolation(literal) {
        found.push((
            "interpolates a raw account id (which embeds an address)",
            matched,
        ));
    }
    found
}

/// `docs/…`, `crates/…`, `clients/…` or `scripts/…`, on a word boundary.
fn repository_path(literal: &str) -> Option<String> {
    for directory in ["docs", "crates", "clients", "scripts"] {
        let mut from = 0;
        while let Some(offset) = literal[from..].find(directory) {
            let at = from + offset;
            let before_is_word = literal[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let after = &literal[at + directory.len()..];
            if !before_is_word && after.starts_with('/') {
                let run = run_of(&after[1..], |c| {
                    c.is_ascii_alphanumeric() || "._/-".contains(c)
                });
                if run > 0 {
                    return Some(literal[at..at + directory.len() + 1 + run].to_owned());
                }
            }
            from = at + 1;
        }
    }
    None
}

/// Any `<name>.md`, which is a design document however it is spelled.
fn document_reference(literal: &str) -> Option<String> {
    let mut from = 0;
    while let Some(offset) = literal[from..].find(".md") {
        let at = from + offset;
        let after = &literal[at + 3..];
        let boundary = !after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let name = run_back(&literal[..at], |c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        });
        if boundary && name > 0 {
            return Some(literal[at - name..at + 3].to_owned());
        }
        from = at + 1;
    }
    None
}

/// `#` followed by two or more digits, where the `#` does not continue an identifier.
fn issue_number(literal: &str) -> Option<String> {
    let mut from = 0;
    while let Some(offset) = literal[from..].find('#') {
        let at = from + offset;
        let before_is_word = literal[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        let digits = run_of(&literal[at + 1..], |c| c.is_ascii_digit());
        if !before_is_word && digits >= 2 {
            return Some(literal[at..at + 1 + digits].to_owned());
        }
        from = at + 1;
    }
    None
}

/// `{account}` or `{account_id}`, with or without a format specifier.
fn account_interpolation(literal: &str) -> Option<String> {
    let mut from = 0;
    while let Some(offset) = literal[from..].find('{') {
        let at = from + offset;
        let inner = &literal[at + 1..];
        for field in ["account_id", "account"] {
            let Some(rest) = inner.strip_prefix(field) else {
                continue;
            };
            if rest.starts_with('}') {
                return Some(literal[at..=at + 1 + field.len()].to_owned());
            }
            if rest.starts_with(':')
                && let Some(close) = rest.find('}')
            {
                return Some(literal[at..=at + 1 + field.len() + close].to_owned());
            }
        }
        from = at + 1;
    }
    None
}

/// How many bytes at the start of `text` satisfy `keep`.
fn run_of(text: &str, keep: impl Fn(char) -> bool) -> usize {
    text.char_indices()
        .find(|(_, c)| !keep(*c))
        .map_or(text.len(), |(i, _)| i)
}

/// How many bytes at the end of `text` satisfy `keep`.
fn run_back(text: &str, keep: impl Fn(char) -> bool) -> usize {
    let mut run = 0;
    for c in text.chars().rev() {
        if !keep(c) {
            break;
        }
        run += c.len_utf8();
    }
    run
}

#[cfg(test)]
mod tests {
    use super::{forbidden, hits, string_literals};

    #[test]
    fn a_logged_repository_path_is_caught() {
        let source = "fn f() {\n    log::error!(\"see docs/provider-oauth.md rule 5\");\n}\n";
        let found = hits("crates/x/src/lib.rs", source);
        assert_eq!(found[0].line, 2);
        assert!(
            found
                .iter()
                .any(|h| h.reason.contains("path in this repository"))
        );
        assert!(found.iter().any(|h| h.reason.contains("design document")));
    }

    #[test]
    fn a_comment_above_a_log_line_is_never_flagged() {
        // This is where our reasoning is supposed to live.
        let source = "// See docs/logging.md for why.\nlog::info!(\"mailbox synced\");\n";
        assert!(hits("crates/x/src/lib.rs", source).is_empty());
    }

    #[test]
    fn an_ordinary_string_outside_a_log_call_is_not_read() {
        let source = "let path = \"docs/logging.md\";\n";
        assert!(hits("crates/x/src/lib.rs", source).is_empty());
    }

    #[test]
    fn every_language_is_scanned() {
        assert!(!hits("a.kt", "logger.error(\"see docs/a.md\")\n").is_empty());
        assert!(!hits("a.cs", "Log.Error(\"see docs/a.md\");\n").is_empty());
        assert!(!hits("a.swift", "NSLog(\"see docs/a.md\")\n").is_empty());
    }

    #[test]
    fn an_issue_number_is_caught_but_a_colour_is_not() {
        assert!(
            forbidden("\"fixed in #1234\"")
                .iter()
                .any(|(r, _)| r.contains("issue"))
        );
        // A single digit is not a ticket, and `#` continuing an identifier is not a reference.
        assert!(forbidden("\"step #1\"").is_empty());
        assert!(forbidden("\"colour ab#12\"").is_empty());
    }

    #[test]
    fn a_raw_account_id_is_caught_with_or_without_a_specifier() {
        assert!(!forbidden("\"syncing {account_id}\"").is_empty());
        assert!(!forbidden("\"syncing {account:?}\"").is_empty());
        // A different field is nobody's business.
        assert!(forbidden("\"syncing {folder}\"").is_empty());
    }

    #[test]
    fn a_wrapped_format_string_is_still_one_literal() {
        let args = "\n    \"a long line \\\n     continued\",\n    count\n";
        assert_eq!(string_literals(args).len(), 1);
    }

    #[test]
    fn an_escaped_quote_does_not_close_a_literal() {
        assert_eq!(string_literals("\"a \\\" b\""), ["\"a \\\" b\""]);
    }

    #[test]
    fn the_next_statement_is_not_read_as_this_calls_arguments() {
        // The blank line bounds a malformed call, so a hit lands on the right line.
        let source = "log::info!(\"fine\");\n\nlet doc = \"docs/logging.md\";\n";
        assert!(hits("crates/x/src/lib.rs", source).is_empty());
    }
}
