//! Fails when swept prose contains a dash used as punctuation.
//!
//! AGENTS.md → "How we write": no em dash, no en dash, no spaced hyphen standing in for one.
//! Rewrite with a comma, a colon, a semicolon, parentheses or a full stop.
//!
//! A dash that is not punctuation stays, and each exemption below is one of those: a range, a minus
//! sign, the cell a capability matrix uses for *not applicable*, a character the rule names in
//! order to forbid it, and every character inside text quoted rather than written.

use std::path::Path;

use crate::{
    git,
    prose::{self, Lines},
};

/// The glyphs a capability matrix is built from.
///
/// True for the legend that declares the convention, for a cell that qualifies it, and for prose
/// naming it. A cell test alone sees only a cell holding nothing else, so without this the legend
/// declaring the convention fails the rule the convention is exempt from.
const MATRIX_GLYPHS: &[char] = &['✅', '🚧', '⬜', '❌'];

/// The dashes the rule is about.
const DASHES: &[char] = &['—', '–'];

/// Line openers that mark captured output or a quoted heading.
const QUOTED_OPENERS: &[&str] = &[">>> ", "> ", "$ ", ">", "OK:", "ERROR:", "WARNING:"];

/// Paths the sweep does not read. Each is either not maintained prose (history, vendored upstream,
/// a generated bundle) or a file whose text is coupled to an assertion somewhere else.
const EXEMPT: &[&str] = &[
    // The rule and the fixtures written to trip it.
    "xtask/src/dash_hygiene.rs",
    "LICENSES/",
    "clients/android/gradle/",
    "clients/composer/dist/",
    "docs/changelog/released/",
    "docs/changelog/announcements/",
    "docs/privacy-policy",
    "crates/mailcal-bindings/src/showcase_data/",
    "crates/mailcal-bindings/src/showcase_bodies/",
    // These values are coupled to assertions in the same test files.
    "clients/windows/uitests/SyncHint.Tests.ps1",
    "clients/windows/uitests/SyncHintBodies.Tests.ps1",
    "crates/mailcal-app/tests/fixtures/imip/README.md",
];

/// Every path the sweep cleaned.
///
/// Leaving one out does not fail the check; it silently stops guarding that half of the tree, which
/// is how the reader-facing docs went unwatched once already. `:(glob)` keeps the last entry to
/// root-level markdown, since a bare `*.md` matches at any depth.
const SWEPT_ROOTS: &[&str] = &[
    "crates",
    "clients",
    "scripts",
    "docs",
    ".agents",
    "messages",
    "branding",
    "allodia_license",
    "docker",
    "xtask",
    ":(glob)*.md",
];

/// The file types the rule reaches.
const EXTENSIONS: &[&str] = &[
    "cs", "html", "js", "kt", "kts", "md", "ps1", "py", "rs", "sh", "swift", "toml", "ts", "xml",
    "yml", "yaml",
];

/// One dash used as punctuation.
#[derive(Debug)]
struct Hit {
    file: String,
    line: usize,
    text: String,
    dash: char,
}

/// Runs the check. `Ok(true)` means no dash is doing a comma's work.
///
/// # Errors
///
/// Propagates a git failure.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let mut checked = 0usize;
    let mut found: Vec<Hit> = Vec::new();

    for name in git::listed(root, SWEPT_ROOTS)? {
        if !is_swept(&name) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&name)) else {
            continue;
        };
        checked += 1;
        found.extend(hits(&name, &text));
    }

    if found.is_empty() {
        println!("OK: swept prose in {checked} file(s) contains no dash punctuation.");
        return Ok(true);
    }

    eprintln!("Dash punctuation remains in swept prose:");
    for hit in &found {
        eprintln!("  {}:{}: {} {}", hit.file, hit.line, hit.dash, hit.text);
    }
    eprintln!("\nERROR: {} dash occurrence(s).", found.len());
    Ok(false)
}

/// Whether the sweep reads this file at all.
fn is_swept(name: &str) -> bool {
    EXTENSIONS.contains(&prose::extension(name)) && !EXEMPT.iter().any(|part| name.contains(part))
}

/// Every dash used as punctuation in one file.
fn hits(name: &str, text: &str) -> Vec<Hit> {
    let markdown = prose::is_markdown(name);
    let mut found = Vec::new();
    for line in Lines::new(text, markdown).collect() {
        if is_quoted_line(line.prose) {
            continue;
        }
        for (index, char) in line.prose.char_indices() {
            if !DASHES.contains(&char) || is_exempt(line.prose, index, char) {
                continue;
            }
            found.push(Hit {
                file: name.to_owned(),
                line: line.number,
                text: line.raw.trim().to_owned(),
                dash: char,
            });
        }
    }
    found
}

/// True when this dash is not punctuation.
fn is_exempt(line: &str, index: usize, dash: char) -> bool {
    if is_range(line, index, dash)
        || is_symbol_reference(line, index)
        || prose::is_quoted(line, index)
        || is_named_character(line, index)
        || prose::is_after_unclosed_backtick(line, index)
    {
        return true;
    }
    dash == '—' && (is_matrix_marker(line) || (line.contains('|') && is_matrix_cell(line, index)))
}

/// A range or a minus sign does not use a dash as sentence punctuation.
///
/// Only the en dash: `A–Z` and `15–120 min` are ranges, and an em dash never is.
fn is_range(line: &str, index: usize, dash: char) -> bool {
    if dash != '–' {
        return false;
    }
    let before = line[..index].chars().next_back();
    let after = line[index + dash.len_utf8()..].chars().next();
    if let (Some(before), Some(after)) = (before, after)
        && !before.is_whitespace()
        && !after.is_whitespace()
    {
        return true;
    }
    // A spaced range between digits, `9 – 5`, which the tight form above does not cover.
    before.is_some_and(char::is_whitespace)
        && after.is_some_and(char::is_whitespace)
        && line[..index]
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| c.is_ascii_digit())
        && line[index + dash.len_utf8()..]
            .chars()
            .nth(1)
            .is_some_and(|c| c.is_ascii_digit())
}

/// Quoted headings and captured output are data, not prose authored here.
fn is_quoted_line(line: &str) -> bool {
    if !line.contains(DASHES) {
        return false;
    }
    let trimmed = line.trim_start();
    if QUOTED_OPENERS.iter().any(|o| trimmed.starts_with(o)) {
        return true;
    }
    // A markdown heading: one to six hashes, then whitespace.
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(char::is_whitespace)
}

/// `(—)` names the character rather than using it, which is how the rule states itself.
fn is_named_character(line: &str, index: usize) -> bool {
    let before = line[..index].chars().next_back();
    let after = line[index..].chars().nth(1);
    before == Some('(') && after == Some(')')
}

/// A dash inside a documented symbol name must remain an exact identifier.
///
/// The three shapes a symbol wears in a comment: a code span, a markdown or Kotlin reference in
/// brackets, and C#'s `<see cref="…"/>` family.
fn is_symbol_reference(line: &str, index: usize) -> bool {
    spans(line, '`', '`').any(|(s, e)| (s..e).contains(&index))
        || spans(line, '[', ']').any(|(s, e)| (s..e).contains(&index))
        || doc_reference_spans(line).any(|(s, e)| (s..e).contains(&index))
}

/// A capability matrix spends the em dash as its "not applicable" cell.
fn is_matrix_marker(line: &str) -> bool {
    MATRIX_GLYPHS.iter().any(|glyph| line.contains(*glyph))
}

/// True when the dash is the whole of the table cell holding it.
fn is_matrix_cell(line: &str, index: usize) -> bool {
    let mut offset = 0;
    for cell in line.split('|') {
        let end = offset + cell.len();
        if (offset..end).contains(&index) {
            return cell.trim() == "—";
        }
        offset = end + 1;
    }
    false
}

/// Every `open … close` span on a line, as byte ranges including the delimiters.
fn spans(line: &str, open: char, close: char) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        loop {
            let start = at + line[at..].find(open)?;
            let after = start + open.len_utf8();
            let Some(offset) = line[after..].find(close) else {
                at = line.len();
                return None;
            };
            let end = after + offset + close.len_utf8();
            at = end;
            // `[]` matches nothing in the original's `[^\]]+`, so an empty pair is not a reference.
            if end > after + close.len_utf8() || open == close {
                return Some((start, end));
            }
        }
    })
}

/// Every `<see …>`, `<paramref …>` or `<typeparamref …>` span on a line.
fn doc_reference_spans(line: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    spans(line, '<', '>').filter(|(start, end)| {
        let inner = &line[start + 1..end - 1];
        ["see", "paramref", "typeparamref"].iter().any(|tag| {
            inner.strip_prefix(*tag).is_some_and(|rest| {
                rest.starts_with(char::is_whitespace) && !rest.trim().is_empty()
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{hits, is_matrix_cell, is_quoted_line, is_range, is_swept};

    #[test]
    fn an_em_dash_in_a_comment_is_caught() {
        let found = hits(
            "crates/x/src/lib.rs",
            "// The gate is a no-op — nothing reaches it.\n",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dash, '—');
    }

    #[test]
    fn a_range_is_not_punctuation() {
        assert!(is_range("A–Z", 1, '–'));
        assert!(is_range("15–120 min", 2, '–'));
        assert!(hits("docs/x.md", "Runs for 15–120 min.\n").is_empty());
    }

    #[test]
    fn a_spaced_en_dash_between_digits_is_still_a_range() {
        assert!(hits("docs/x.md", "Open 9 – 5 daily.\n").is_empty());
    }

    #[test]
    fn a_dash_inside_a_symbol_stays() {
        assert!(hits("docs/x.md", "The `some—name` field.\n").is_empty());
        assert!(hits("docs/x.md", "See [a—reference] for it.\n").is_empty());
        assert!(hits("crates/x/src/a.cs", "// <see cref=\"A—B\"/> names it\n").is_empty());
    }

    #[test]
    fn the_rule_may_name_the_character_it_forbids() {
        assert!(hits("docs/x.md", "Not an em dash (—), not an en dash (–).\n").is_empty());
    }

    #[test]
    fn a_matrix_cell_keeps_its_not_applicable_marker() {
        assert!(is_matrix_cell("| a | — | b |", 6));
        assert!(hits("docs/x.md", "| Windows | — | ✅ |\n").is_empty());
    }

    #[test]
    fn quoted_output_keeps_its_typography() {
        assert!(is_quoted_line("> a quoted — line"));
        assert!(is_quoted_line("### A heading — with a dash"));
        assert!(hits("docs/x.md", "$ tool --flag — output\n").is_empty());
        assert!(hits("docs/x.md", "Says \"one — two\" exactly.\n").is_empty());
    }

    #[test]
    fn code_outside_a_comment_is_not_prose() {
        assert!(hits("crates/x/src/lib.rs", "let s = \"a — b\";\n").is_empty());
    }

    #[test]
    fn a_fenced_block_is_not_prose() {
        assert!(hits("docs/x.md", "before\n```\na — b\n```\nafter\n").is_empty());
    }

    #[test]
    fn the_sweep_reads_what_it_says_it_does() {
        assert!(is_swept("docs/x.md"));
        assert!(!is_swept("docs/privacy-policy.md"));
        assert!(!is_swept("docs/changelog/released/0.8.2.md"));
        assert!(!is_swept("docs/x.png"));
    }
}
