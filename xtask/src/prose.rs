//! Deciding which part of a file the writing rules reach.
//!
//! The rules in AGENTS.md → "How we write" are about prose: markdown outside fenced blocks, and
//! comment lines in source. Code is not prose, and neither is a fenced example, so both checkers
//! that enforce a writing rule need the same answer to "what is the prose on this line" before they
//! can differ about what they look for in it.
//!
//! The shapes here are the ones the shell and Python originals matched with regular expressions.
//! They are spelled out rather than pattern-matched because this crate has no dependencies, and
//! because each is small enough that the code says what the pattern meant.

use std::borrow::Cow;

/// The comment openers a prose line may start with, longest first where one is a prefix of another.
///
/// The order is load-bearing. `/**` has to be tried before `/*`, or a KDoc block's body keeps a
/// leading `*` that the rules would then read as prose. `///` and `//!` have to be tried before
/// `//` for the same reason.
const OPENERS: &[&str] = &["/**", "/*", "///", "//!", "//", "#", "*", "--", "<!--"];

/// The text after a line's comment opener, or `None` when the line is not a comment.
///
/// At most one space is eaten after the opener, matching the `\s?` the originals used: a comment
/// indented for a list or a code example keeps the indentation that makes it one.
pub(crate) fn comment_body(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let opener = OPENERS.iter().find(|o| trimmed.starts_with(**o))?;
    let rest = &trimmed[opener.len()..];
    Some(match rest.chars().next() {
        Some(c) if c.is_whitespace() => &rest[c.len_utf8()..],
        _ => rest,
    })
}

/// A path's extension, without the dot, or `""` when it has none.
///
/// Read off the name rather than through `Path::extension`, so the comparison stays a plain string
/// one and a checker's file-type list can be written the way a reader expects to see it.
pub(crate) fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(_, e)| e)
}

/// True when the rules should read the whole of this file rather than only its comments.
pub(crate) fn is_markdown(name: &str) -> bool {
    extension(name) == "md"
}

/// True when the line opens or closes a markdown fenced block.
pub(crate) fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

/// The line with every `` `…` `` span removed.
///
/// An unclosed backtick opens a span that runs to the end of the line, which is what the original's
/// `` `[^`]*` `` did by simply not matching: the tail survives. Here the tail is kept for the same
/// reason, so a wrapped quotation reads the same to both checkers.
pub(crate) fn without_code_spans(line: &str) -> Cow<'_, str> {
    if !line.contains('`') {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            out.push_str(rest);
            return Cow::Owned(out);
        };
        out.push_str(&rest[..open]);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Walks a file's lines, yielding the ones a writing rule reaches.
///
/// Markdown is prose outside its fenced blocks; every other language is prose only inside a
/// comment. The line number is the file's, so a report points where the reader has to go.
#[derive(Debug)]
pub(crate) struct Lines<'a> {
    text: &'a str,
    markdown: bool,
}

/// One line the rules reach: where it is, what the file says, and the prose on it.
#[derive(Debug)]
pub(crate) struct Line<'a> {
    /// One-based, as an editor counts.
    pub(crate) number: usize,
    /// The line exactly as the file holds it, for reporting.
    pub(crate) raw: &'a str,
    /// The prose: the whole line in markdown, the comment body elsewhere.
    pub(crate) prose: &'a str,
}

impl<'a> Lines<'a> {
    /// Reads `text`, treating it as markdown when `markdown`.
    pub(crate) fn new(text: &'a str, markdown: bool) -> Self {
        Self { text, markdown }
    }

    /// Every line a writing rule reaches.
    pub(crate) fn collect(&self) -> Vec<Line<'a>> {
        let mut out = Vec::new();
        let mut fenced = false;
        for (index, raw) in self.text.lines().enumerate() {
            if self.markdown && is_fence(raw) {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            let prose = if self.markdown {
                Some(raw)
            } else {
                comment_body(raw)
            };
            if let Some(prose) = prose {
                out.push(Line {
                    number: index + 1,
                    raw,
                    prose,
                });
            }
        }
        out
    }
}

/// True when the character at `index` sits inside a `"…"` or `'…'` span.
///
/// Quoted headings and captured output are data, not prose authored here, so their typography is
/// theirs. The spans are non-greedy and must close, which is what keeps an apostrophe in ordinary
/// prose from opening one that swallows the rest of the line.
pub(crate) fn is_quoted(text: &str, index: usize) -> bool {
    let mut at = 0;
    while let Some(quote) = text[at..].chars().next() {
        let after = at + quote.len_utf8();
        if quote != '"' && quote != '\'' {
            at = after;
            continue;
        }
        match text[after..].find(quote) {
            Some(offset) => {
                let end = after + offset;
                if (at..=end).contains(&index) {
                    return true;
                }
                at = end + quote.len_utf8();
            }
            // ⚠️ An unpartnered quote opens nothing, and the scan has to carry on past it rather
            // than give up. An apostrophe is the common one: in `the header's title, "Jun – Jul"`
            // it comes first, and a reader that stopped there would never see the span that
            // actually holds the dash. The regular expression this replaces simply failed to match
            // at the apostrophe and tried the next position.
            None => at = after,
        }
    }
    false
}

/// True when an odd number of backticks precede `index`, so it sits in a span that wraps.
pub(crate) fn is_after_unclosed_backtick(text: &str, index: usize) -> bool {
    text[..index].matches('`').count() % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::{
        Lines, comment_body, is_after_unclosed_backtick, is_fence, is_quoted, without_code_spans,
    };

    #[test]
    fn reads_a_comment_body_past_the_longest_opener() {
        // `/**` before `/*`, or the body keeps a `*` the rules would read as prose.
        assert_eq!(comment_body("/** a doc */"), Some("a doc */"));
        assert_eq!(comment_body("  /// a doc"), Some("a doc"));
        assert_eq!(comment_body("//! a module doc"), Some("a module doc"));
        assert_eq!(comment_body("# a script"), Some("a script"));
    }

    #[test]
    fn only_one_space_is_eaten_after_the_opener() {
        // An indented example inside a comment keeps the indentation that makes it one.
        assert_eq!(comment_body("//     indented"), Some("    indented"));
    }

    #[test]
    fn code_is_not_a_comment() {
        assert_eq!(comment_body("let x = 1;"), None);
        assert_eq!(comment_body(""), None);
    }

    #[test]
    fn strips_code_spans_but_keeps_an_unclosed_one() {
        assert_eq!(without_code_spans("a `b` c"), "a  c");
        assert_eq!(without_code_spans("a `b c"), "a `b c");
        assert_eq!(without_code_spans("`a` and `b`"), " and ");
    }

    #[test]
    fn markdown_prose_stops_at_a_fence() {
        let text = "before\n```\ninside\n```\nafter\n";
        let lines = Lines::new(text, true).collect();
        let seen: Vec<&str> = lines.iter().map(|l| l.prose).collect();
        assert_eq!(seen, ["before", "after"]);
        // The number is the file's, so a report points where the reader has to go.
        assert_eq!(lines[1].number, 5);
    }

    #[test]
    fn source_prose_is_the_comments_only() {
        let text = "fn main() {\n    // a note\n}\n";
        let lines = Lines::new(text, false).collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].prose, "a note");
        assert_eq!(lines[0].number, 2);
    }

    #[test]
    fn a_fence_only_closes_in_markdown() {
        // A Rust line starting with ``` is inside a doc comment, and the comment reader sees it.
        let text = "/// ```\n/// code\n/// ```\n";
        assert_eq!(Lines::new(text, false).collect().len(), 3);
    }

    #[test]
    fn quoted_spans_are_found_and_an_unclosed_one_is_not() {
        let text = "say \"one\" then two";
        assert!(is_quoted(text, 5));
        assert!(!is_quoted(text, 12));
        assert!(!is_quoted("it's fine", 4));
    }

    #[test]
    fn an_apostrophe_does_not_hide_the_span_after_it() {
        // The shape that caught this out: an unpartnered apostrophe earlier on the line, and the
        // quoted span holding what the caller is asking about later. Giving up at the apostrophe
        // reports the span as unquoted.
        let text = "the header's title, \"Jun - Jul\"";
        let dash = text.find('-').unwrap();
        assert!(is_quoted(text, dash));
    }

    #[test]
    fn counts_backticks_before_a_position() {
        assert!(is_after_unclosed_backtick("a `b c", 5));
        assert!(!is_after_unclosed_backtick("a `b` c", 6));
    }

    #[test]
    fn a_fence_is_recognised_indented() {
        assert!(is_fence("  ```rust"));
        assert!(!is_fence("a ``` b"));
    }
}
