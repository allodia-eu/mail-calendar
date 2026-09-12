//! Fails when prose in this repository is written in American English.
//!
//! AGENTS.md → "How we write" states the rule: British English in prose, comments and user-facing
//! copy alike, and identifiers keep whatever spelling their spec or tool gave them. The tree was
//! swept once to satisfy it, which is exactly the state a rule rots from: a hundred files agree,
//! nobody remembers why, and the next `behavior` reads like precedent. This is the machine half.
//!
//! **What counts as prose.** Markdown outside fenced blocks, and comment lines in source. Inline
//! code spans are cut out before matching, and a hit sitting against an identifier character is
//! ignored, so `normalizeColor` and `authorization_url` are invisible here whatever file they are
//! in.
//!
//! **What the rule deliberately does not reach** is the vocabulary in
//! [`crate::british_english_words`]: OAuth's and iCalendar's own terms, toolkit and language
//! keywords, and three words that are the thing's actual name rather than a spelling choice
//! (`dialog`, `artifact`, `catalog`). A name is not a spelling, and a repository that "corrected"
//! `ORGANIZER` would be describing a property that does not exist.
//!
//! **A doc reference is an identifier wearing prose clothes**, and the reason `cref=` is a symbol:
//! C#'s `<see cref="Maximized"/>` and Kotlin's `[authorizationUrl]` name a symbol from inside a
//! comment, where nothing backticks them and renaming one resolves to nothing.

use std::path::Path;

use crate::{
    british_english_words::{EXEMPT, EXTENSIONS, PHRASES, SPELLINGS, SYMBOLS},
    git,
    prose::{self, Lines},
};

/// How far either side of a hit the exempting vocabulary is looked for.
const WINDOW: usize = 30;

/// One American spelling, where it is and what it should be.
#[derive(Debug)]
struct Hit {
    file: String,
    line: usize,
    found: String,
    wanted: &'static str,
}

/// Runs the check. `Ok(true)` means every line of prose is British English.
///
/// # Errors
///
/// Propagates a git failure: a listing that came back empty would be a check that cannot fail.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let files = git::listed(root, &[])?;
    let mut checked = 0usize;
    let mut found: Vec<Hit> = Vec::new();

    for name in files {
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
        println!("OK: prose in {checked} file(s) is British English.");
        return Ok(true);
    }

    eprintln!("Prose written in American English:");
    for hit in &found {
        eprintln!(
            "  {}:{}: {} -> {}",
            hit.file, hit.line, hit.found, hit.wanted
        );
    }
    eprintln!(
        "\nERROR: {} spelling(s). AGENTS.md -> \"How we write\": British English in prose,\
         \ncomments and user-facing copy. If the word is a name somebody else chose, an RFC's,\
         \na toolkit's, a doc reference to a symbol, backtick it, or add it to PHRASES/SYMBOLS in\
         \nxtask/src/british_english_words.rs.",
        found.len()
    );
    Ok(false)
}

/// Whether the rule reaches this file at all.
fn is_swept(name: &str) -> bool {
    EXTENSIONS.contains(&prose::extension(name)) && !EXEMPT.iter().any(|part| name.contains(part))
}

/// Every American spelling in one file's prose.
///
/// One pass over each line, not one per spelling. Searching the line once per entry is how the
/// original spent eighteen seconds on this tree: a hundred and twenty scans of every line of prose
/// in the repository. Walking the words instead and looking each one up costs a single pass, and
/// the lookup is exact because every entry is a whole word by construction.
fn hits(name: &str, text: &str) -> Vec<Hit> {
    let markdown = prose::is_markdown(name);
    let mut found = Vec::new();
    // Where each spelling first appears on the line under inspection. Only the first, because that
    // is the match the rule's exemptions are judged against.
    let mut first: Vec<Option<usize>> = vec![None; SPELLINGS.len()];

    for line in Lines::new(text, markdown).collect() {
        let cleaned = prose::without_code_spans(line.prose);
        first.fill(None);
        for (at, word) in words(&cleaned) {
            if let Some(index) = spelling_index(word)
                && first[index].is_none()
            {
                first[index] = Some(at);
            }
        }
        for (index, at) in first
            .iter()
            .enumerate()
            .filter_map(|(i, a)| Some((i, (*a)?)))
        {
            let (american, british) = SPELLINGS[index];
            if is_excused(&cleaned, at, american.len()) {
                continue;
            }
            found.push(Hit {
                file: name.to_owned(),
                line: line.number,
                found: cleaned[at..at + american.len()].to_owned(),
                wanted: british,
            });
        }
    }
    found
}

/// Every maximal run of word characters on a line, with where it starts.
///
/// A run is exactly what a regular expression's `\b…\b` delimits, which is why an entry matches
/// only when it is the whole run: `colorScheme` and `recolor` are one run each and neither is
/// `color`.
fn words(line: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let start = at + line[at..].find(is_word_char)?;
        let end = line[start..]
            .find(|c: char| !is_word_char(c))
            .map_or(line.len(), |offset| start + offset);
        at = end;
        Some((start, &line[start..end]))
    })
}

/// Which entry in [`SPELLINGS`] this word is, if any.
///
/// Bucketed by first letter so a word is compared against a handful of entries rather than all of
/// them. The buckets are built once, on first use.
fn spelling_index(word: &str) -> Option<usize> {
    static BUCKETS: std::sync::OnceLock<Vec<Vec<usize>>> = std::sync::OnceLock::new();
    let buckets = BUCKETS.get_or_init(|| {
        let mut buckets = vec![Vec::new(); 26];
        for (index, (american, _)) in SPELLINGS.iter().enumerate() {
            let letter = american.as_bytes()[0] - b'a';
            buckets[usize::from(letter)].push(index);
        }
        buckets
    });
    let first = word.as_bytes().first()?.to_ascii_lowercase();
    let bucket = buckets.get(usize::from(first.checked_sub(b'a')?))?;
    bucket
        .iter()
        .copied()
        .find(|index| SPELLINGS[*index].0.eq_ignore_ascii_case(word))
}

/// What a regular expression's `\w` means: a letter, a digit or an underscore.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True when the hit is an identifier or somebody else's vocabulary rather than our prose.
///
/// The full stop is the interesting one. It ends a sentence far more often than it joins
/// `body.center.x`, so it excuses a hit only with a word character behind it.
///
/// ⚠️ A spelling at the **end of a line** is judged like any other. The Python original wrote this
/// as `after[:1] in "_("`, and in Python the empty string is a substring of every string, so a
/// match with nothing after it was excused: wrapped prose, which is where most of them land, was
/// invisible to the rule. Forty-five of them were in the tree when this was ported.
fn is_excused(line: &str, start: usize, length: usize) -> bool {
    let end = start + length;
    let before = line[..start].chars().next_back();
    let mut after = line[end..].chars();
    let first = after.next();
    let second = after.next();

    if before == Some('_') {
        return true;
    }
    if before == Some('.')
        && line[..start - 1]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
    {
        return true;
    }
    if first == Some('_') || first == Some('(') {
        return true;
    }
    if first == Some('.') && second.is_some_and(char::is_alphanumeric) {
        return true;
    }
    // A code span that wraps to the next line: the closing backtick is not on this one, so the
    // span was never cut out and its tail reads as prose. A parameter list broken across two
    // comment lines is what produces one, and the identifier in it is not ours to respell. The
    // dash rule exempts the same shape; this rule needs it only because a hit at the end of a line
    // is judged at all, and a wrapped span is the one thing that can put an identifier there.
    if prose::is_after_unclosed_backtick(line, start) {
        return true;
    }

    let window = window_around(line, start, end);
    if SYMBOLS.iter().any(|symbol| window.contains(symbol)) {
        return true;
    }
    let flattened = window.to_lowercase().replace('-', " ");
    PHRASES.iter().any(|phrase| flattened.contains(phrase))
}

/// The text within [`WINDOW`] characters either side of a hit, on character boundaries.
fn window_around(line: &str, start: usize, end: usize) -> &str {
    let from = line[..start]
        .char_indices()
        .nth_back(WINDOW.saturating_sub(1))
        .map_or(0, |(i, _)| i);
    let to = line[end..]
        .char_indices()
        .nth(WINDOW)
        .map_or(line.len(), |(i, _)| end + i);
    &line[from..to]
}

#[cfg(test)]
mod tests {
    use super::{hits, is_swept, spelling_index, words};

    /// The words a line breaks into, with where each starts.
    fn split(line: &str) -> Vec<(usize, &str)> {
        words(line).collect()
    }

    #[test]
    fn a_line_breaks_into_the_runs_a_word_boundary_delimits() {
        assert_eq!(split("a color."), [(0, "a"), (2, "color")]);
        assert_eq!(
            split("body.center.x"),
            [(0, "body"), (5, "center"), (12, "x")]
        );
    }

    #[test]
    fn a_spelling_is_recognised_whatever_its_case() {
        assert!(spelling_index("behavior").is_some());
        assert!(spelling_index("Behavior").is_some());
        assert!(spelling_index("BEHAVIOR").is_some());
    }

    #[test]
    fn an_identifier_is_not_a_word() {
        // A run has to be the whole entry, which is what keeps `colorScheme` and `recolor` out of
        // a search for `color`: each is one run, and neither run is `color`.
        assert_eq!(split("colorScheme"), [(0, "colorScheme")]);
        assert!(spelling_index("colorScheme").is_none());
        assert!(spelling_index("recolor").is_none());
    }

    #[test]
    fn a_comment_in_american_english_is_caught() {
        let found = hits("crates/x/src/lib.rs", "// The behavior here is odd.\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].wanted, "behaviour");
        assert_eq!(found[0].line, 1);
    }

    #[test]
    fn code_outside_a_comment_is_not_prose() {
        assert!(hits("crates/x/src/lib.rs", "let behavior = 1;\n").is_empty());
    }

    #[test]
    fn a_code_span_is_cut_out_before_matching() {
        assert!(hits("docs/x.md", "The `behavior` field is read.\n").is_empty());
    }

    #[test]
    fn a_fenced_block_is_not_prose() {
        let text = "before\n```\nlet behavior = 1;\n```\nafter\n";
        assert!(hits("docs/x.md", text).is_empty());
    }

    #[test]
    fn an_identifier_touching_the_word_excuses_it() {
        assert!(hits("crates/x/src/lib.rs", "// reads body.center.x here\n").is_empty());
        assert!(hits("crates/x/src/lib.rs", "// calls center() on it\n").is_empty());
        assert!(hits("crates/x/src/lib.rs", "// the item_color value\n").is_empty());
    }

    #[test]
    fn a_sentence_ending_in_the_word_is_still_caught() {
        // The full stop ends a sentence far more often than it joins an identifier, and a hit at
        // the end of a line is the shape that used to escape the rule entirely.
        let found = hits(
            "crates/x/src/lib.rs",
            "// Nothing here changes the behavior\n",
        );
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn somebody_elses_vocabulary_is_left_alone() {
        assert!(
            hits(
                "crates/x/src/lib.rs",
                "// the authorization endpoint answers\n"
            )
            .is_empty()
        );
        assert!(hits("crates/x/src/lib.rs", "// ORGANIZER is the property name\n").is_empty());
    }

    #[test]
    fn the_rule_reaches_the_file_types_it_says_it_does() {
        assert!(is_swept("docs/x.md"));
        assert!(is_swept("crates/x/src/lib.rs"));
        assert!(!is_swept("docs/x.png"));
        assert!(!is_swept("LICENSES/GPL-3.0-only.txt"));
        assert!(!is_swept("docs/privacy-policy.md"));
    }
}
