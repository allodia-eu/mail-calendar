//! The git queries the checks are built on, and the small text helpers that read their output.
//!
//! Searching is delegated to `git grep` rather than reimplemented. Three reasons, in the order they
//! matter: it decides tracked-versus-ignored the way the rest of the repository does, so a check
//! never descends into `target/` or the generated bindings; its POSIX extended regular expressions
//! are the ones the rules were written against, and a second engine with slightly different
//! semantics would quietly change what a rule forbids; and it is one process per pattern rather
//! than one per file, which is where the cost of a check actually lives.

use std::{path::Path, process::Command};

/// What a search found.
#[derive(Debug)]
pub(crate) struct Matches {
    /// The matching lines, as git printed them.
    pub(crate) lines: Vec<String>,
}

impl Matches {
    /// True when nothing matched.
    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The matches with every line containing `needle` removed.
    pub(crate) fn without_lines_containing(self, needle: &str) -> Self {
        Self {
            lines: self
                .lines
                .into_iter()
                .filter(|l| !l.contains(needle))
                .collect(),
        }
    }

    /// The matches with every line satisfying `drop` removed.
    pub(crate) fn without(self, drop: impl Fn(&str) -> bool) -> Self {
        Self {
            lines: self.lines.into_iter().filter(|l| !drop(l)).collect(),
        }
    }

    /// The distinct file names among the matches, in the order they first appear.
    pub(crate) fn files(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for line in &self.lines {
            let name = line.split(':').next().unwrap_or(line).to_owned();
            if !seen.contains(&name) {
                seen.push(name);
            }
        }
        seen
    }
}

/// How a pattern is interpreted.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    /// A literal string (`git grep -F`).
    Fixed,
    /// A POSIX extended regular expression (`git grep -E`).
    Extended,
}

/// Runs `git grep` and returns the matching lines.
///
/// Untracked files are searched: without that, a new file is invisible until it is staged, so a
/// check passes on the very working tree that introduces the thing it forbids and fails on the
/// trunk once the file is committed. Ignored paths stay ignored either way.
///
/// # Errors
///
/// Fails when git cannot be run, or when it exits with anything other than "found" (0) or "not
/// found" (1). That distinction is the point: an error that reads as "no matches" is how a check
/// passes while the thing it bans sits in the tree.
pub(crate) fn grep(
    root: &Path,
    kind: Kind,
    pattern: &str,
    pathspecs: &[&str],
) -> Result<Matches, String> {
    let mut command = Command::new("git");
    command.arg("grep").arg("-I").arg("-n").arg("--untracked");
    command.arg(match kind {
        Kind::Fixed => "-F",
        Kind::Extended => "-E",
    });
    command.arg(pattern);
    // The pathspecs go after a single `--`, or git reads the first one as a revision.
    command.arg("--");
    if pathspecs.is_empty() {
        command.arg(".");
    } else {
        command.args(pathspecs);
    }

    let output = command
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git grep: {e}"))?;

    match output.status.code() {
        Some(0) => Ok(Matches {
            lines: String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect(),
        }),
        Some(1) => Ok(Matches { lines: Vec::new() }),
        other => Err(format!(
            "git grep exited {}: {}",
            other.map_or_else(|| "on a signal".to_owned(), |c| c.to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// True when `pattern` appears anywhere in `pathspec`.
///
/// # Errors
///
/// Propagates a git failure, as [`grep`] does.
pub(crate) fn contains(root: &Path, pattern: &str, pathspec: &str) -> Result<bool, String> {
    Ok(!grep(root, Kind::Fixed, pattern, &[pathspec])?.is_empty())
}

/// Every tracked path matching `pathspecs`, as git reports it.
///
/// # Errors
///
/// Fails when git cannot be run or reports an error.
pub(crate) fn ls_files(root: &Path, pathspecs: &[&str]) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("ls-files")
        .arg("-z")
        .args(pathspecs)
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git ls-files: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Reads a file, or explains which one could not be read.
///
/// # Errors
///
/// Fails when the file is missing or unreadable.
pub(crate) fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative))
        .map_err(|e| format!("could not read {relative}: {e}"))
}

/// The words inside a shell array assignment, `NAME=(a b c)`.
///
/// Quotes and commas are stripped and the words are sorted and deduplicated, matching what the
/// shell original did with `tr` and `sort -u`.
pub(crate) fn array_words(haystack: &str, name: &str) -> Vec<String> {
    let prefix = format!("{name}=(");
    for line in haystack.lines() {
        let line = line.trim_end();
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        if let Some(inner) = rest.strip_suffix(')') {
            return sorted_words(inner);
        }
    }
    Vec::new()
}

/// Splits on whitespace, `|` and `,`, strips quotes, sorts and deduplicates.
///
/// The comma is a separator rather than a character to strip from the ends. A `ValidateSet` written
/// without spaces, `'en','nl'`, is one word to a splitter that only trims, and the list then
/// silently parses to a single nonsense entry that matches nothing.
pub(crate) fn sorted_words(text: &str) -> Vec<String> {
    let mut words: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == '|' || c == ',')
        .map(|w| w.trim_matches(|c| c == '\'' || c == '"').to_owned())
        .filter(|w| !w.is_empty())
        .collect();
    words.sort();
    words.dedup();
    words
}

/// The body of a shell function, from `name() {` to the closing brace in column one.
pub(crate) fn function_body<'a>(haystack: &'a str, name: &str) -> &'a str {
    let opener = format!("{name}() {{");
    let Some(start) = haystack.find(&opener) else {
        return "";
    };
    let rest = &haystack[start..];
    rest.find("\n}").map_or(rest, |end| &rest[..end])
}

#[cfg(test)]
mod tests {
    use super::{array_words, function_body, sorted_words};

    #[test]
    fn reads_a_shell_array() {
        let text = "before\nALL_LOCALES=(en nl de)\nafter\n";
        assert_eq!(array_words(text, "ALL_LOCALES"), ["de", "en", "nl"]);
    }

    #[test]
    fn an_absent_array_is_empty_not_an_error() {
        // A parse that finds nothing must be visible to the caller, which compares against empty.
        assert!(array_words("nothing here\n", "ALL_SCREENS").is_empty());
    }

    #[test]
    fn strips_quotes_commas_and_splits_on_pipes() {
        assert_eq!(sorted_words("'en', \"nl\"|de"), ["de", "en", "nl"]);
    }

    #[test]
    fn takes_a_function_body_up_to_the_closing_brace() {
        let text = "x() {\n  inner\n}\nafter() {\n  other\n}\n";
        assert!(function_body(text, "x").contains("inner"));
        assert!(!function_body(text, "x").contains("other"));
    }
}
