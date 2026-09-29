//! Cutting a sent message down to the words its author wrote: no quoted original, no forwarded
//! message, no signature block.
//!
//! Works over plain text, line by line, and cuts at the **first** line that starts something the
//! author did not write, because everything below such a line is someone else's (or the author's
//! own older mail). The shapes it recognises, in the order the plan of a reply puts them:
//!
//! 1. The RFC 3676 signature delimiter, `-- ` (and the bare `--` a client that trims trailing space
//!    leaves behind).
//! 2. An attribution line in any catalog locale's wording ("On {date}, {sender} wrote:"), read from
//!    `messages/*.json` at build time, plus the few foreign shapes listed below. An attribution a
//!    client wrapped over two lines is recognised too.
//! 3. A forwarded-message line: the catalog's own label, which is what this app writes above a
//!    forward, or any dash-framed marker (`---------- Forwarded message ---------`, `-----Original
//!    Message-----`), whatever language is between the dashes.
//! 4. A header block (`From:` then `Sent:`, in any catalog locale), with or without the rule line
//!    Outlook draws above it.
//! 5. Three `>`-quoted lines in a row. A lone quoted line inside the author's own text is dropped
//!    and the text around it kept.
//!
//! What is left loses a trailing copy of the account's own signature, for mail sent by a client
//! that wrote no delimiter.

use crate::catalog::{CATALOG, LocaleShapes};

/// Attribution shapes of other clients that no catalog locale writes, in the catalog's
/// placeholder syntax.
const FOREIGN_ATTRIBUTIONS: [&str; 1] = [
    // Apple Mail in Dutch.
    "Op {date} heeft {sender} het volgende geschreven:",
];

/// Header labels no catalog locale uses but other clients do.
const FOREIGN_HEADERS: [&str; 2] = ["Date", "Datum"];

/// The longest line taken for an attribution. A real one is a date and a name; a longer line is
/// prose that happens to open with the same word.
const ATTRIBUTION_MAX_CHARS: usize = 240;

/// The shapes to cut at, compiled once per corpus.
pub(crate) struct Stripper {
    attributions: Vec<Vec<String>>,
    forwarded: Vec<String>,
    from_labels: Vec<String>,
    header_labels: Vec<String>,
}

impl Stripper {
    pub(crate) fn new() -> Self {
        Self::from_catalog(CATALOG)
    }

    fn from_catalog(catalog: &[LocaleShapes]) -> Self {
        let attributions = catalog
            .iter()
            .map(|shapes| shapes.attribution)
            .chain(FOREIGN_ATTRIBUTIONS)
            .map(segments)
            .collect();
        let forwarded = catalog
            .iter()
            .map(|shapes| normalise(shapes.forwarded).to_lowercase())
            .collect();
        let from_labels = catalog
            .iter()
            .map(|shapes| normalise(shapes.headers[0]).to_lowercase())
            .collect();
        let header_labels = catalog
            .iter()
            .flat_map(|shapes| shapes.headers.iter().copied())
            .chain(FOREIGN_HEADERS)
            .map(|label| normalise(label).to_lowercase())
            .collect();
        Self {
            attributions,
            forwarded,
            from_labels,
            header_labels,
        }
    }

    /// The part of `body` its author wrote, given the plain text of each signature the account
    /// has used.
    pub(crate) fn own_text(&self, body: &str, signatures: &[String]) -> String {
        let lines: Vec<String> = body.lines().map(normalise).collect();
        let cut = (0..lines.len())
            .find(|&index| self.starts_foreign_text(&lines, index))
            .unwrap_or(lines.len());
        let mut kept: Vec<&str> = lines[..cut]
            .iter()
            .map(String::as_str)
            .filter(|line| !line.trim_start().starts_with('>'))
            .collect();
        for signature in signatures {
            drop_trailing_signature(&mut kept, signature);
        }
        collapse_blank_lines(&kept)
    }

    /// Whether the line at `index` starts text the author did not write.
    fn starts_foreign_text(&self, lines: &[String], index: usize) -> bool {
        let line = lines[index].trim();
        if line == "--" {
            return true;
        }
        let next = lines.get(index + 1).map_or("", |line| line.trim());
        if self.is_attribution(line)
            || (!next.is_empty() && self.is_attribution(&format!("{line} {next}")))
        {
            return true;
        }
        if is_dash_framed(line)
            || self
                .forwarded
                .contains(&line.trim_matches('-').trim().to_lowercase())
        {
            return true;
        }
        let following = following_non_empty(lines, index);
        if is_rule(line) && following.first().is_some_and(|line| self.is_header(line)) {
            return true;
        }
        if self.is_from_header(line) && following.first().is_some_and(|line| self.is_header(line)) {
            return true;
        }
        lines[index..]
            .iter()
            .take(3)
            .filter(|line| line.trim_start().starts_with('>'))
            .count()
            == 3
    }

    fn is_attribution(&self, line: &str) -> bool {
        line.chars().count() <= ATTRIBUTION_MAX_CHARS
            && line.chars().any(|ch| ch.is_ascii_digit())
            && self
                .attributions
                .iter()
                .any(|segments| matches_segments(line, segments))
    }

    fn is_from_header(&self, line: &str) -> bool {
        header_label(line).is_some_and(|label| self.from_labels.contains(&label))
    }

    fn is_header(&self, line: &str) -> bool {
        header_label(line).is_some_and(|label| self.header_labels.contains(&label))
    }
}

/// A catalog template split at its `{placeholders}` into the literal text between them.
fn segments(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = normalise(template);
    while let Some(open) = rest.find('{') {
        out.push(rest[..open].to_lowercase());
        rest = rest[open..]
            .split_once('}')
            .map_or_else(String::new, |(_, tail)| tail.to_owned());
    }
    out.push(rest.to_lowercase());
    out
}

/// Whether `line` is the template's literal text in order, with something in every gap: it
/// starts with the first segment, ends with the last, and holds the rest in between.
fn matches_segments(line: &str, segments: &[String]) -> bool {
    let line = line.to_lowercase();
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return false;
    };
    if segments.len() < 2 || !line.starts_with(first.as_str()) || !line.ends_with(last.as_str()) {
        return false;
    }
    let Some(mut rest) = line
        .get(first.len()..line.len() - last.len())
        .map(str::to_owned)
    else {
        return false;
    };
    for middle in &segments[1..segments.len() - 1] {
        match rest.find(middle.as_str()) {
            Some(at) if at > 0 => rest = rest[at + middle.len()..].to_owned(),
            _ => return false,
        }
    }
    !rest.trim().is_empty()
}

/// `-----Original Message-----`, `---------- Forwarded message ---------`: words framed by runs
/// of three or more dashes.
fn is_dash_framed(line: &str) -> bool {
    let inner = line.trim_matches('-');
    line.starts_with("---")
        && line.ends_with("---")
        && inner.chars().any(char::is_alphabetic)
        && line.len() - inner.len() >= 6
}

/// A rule line: ten or more underscores or dashes and nothing else.
fn is_rule(line: &str) -> bool {
    line.chars().count() >= 10 && line.chars().all(|ch| ch == '_' || ch == '-')
}

/// The label of a `Label: value` line, lower-cased, when the line has that shape.
fn header_label(line: &str) -> Option<String> {
    let (label, _) = line.trim_start_matches('*').split_once(':')?;
    let label = label.trim().trim_end_matches('*');
    (!label.is_empty() && label.chars().count() <= 20 && !label.contains(' '))
        .then(|| label.to_lowercase())
}

/// The next two non-empty lines after `index`.
fn following_non_empty(lines: &[String], index: usize) -> Vec<&str> {
    lines[index + 1..]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .take(2)
        .collect()
}

/// Removes the account's signature from the end of `kept`, when the last non-empty lines are
/// exactly its lines.
fn drop_trailing_signature(kept: &mut Vec<&str>, signature: &str) {
    let wanted: Vec<String> = signature
        .lines()
        .map(|line| normalise(line).trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let written: Vec<usize> = (0..kept.len())
        .filter(|&index| !kept[index].trim().is_empty())
        .collect();
    if wanted.is_empty() || written.len() < wanted.len() {
        return;
    }
    let tail = &written[written.len() - wanted.len()..];
    if tail
        .iter()
        .zip(&wanted)
        .all(|(&index, line)| kept[index].trim() == line)
    {
        kept.truncate(tail[0]);
    }
}

/// Joins `lines`, keeping at most one blank line in a row, trimmed.
fn collapse_blank_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut blank = false;
    for line in lines {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if blank {
            out.push('\n');
            blank = false;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// Every whitespace character as a plain space (a French attribution carries a no-break space
/// before its colon), and the line's trailing space gone, which turns a signature delimiter's
/// `-- ` into `--`.
fn normalise(line: &str) -> String {
    let spaced: String = line
        .chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .collect();
    spaced.trim_end().to_owned()
}

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
