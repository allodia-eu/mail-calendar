//! A message's text with what costs a prompt tokens and tells a model nothing taken out: carriage
//! returns, trailing spaces, runs of blank lines, and the `<https://…>` a plain-text conversion
//! writes after every link's text. A long signature is mostly these.

/// `text` with one newline per line end, no trailing spaces, at most one blank line in a row, and
/// no `<http…>` or `<mailto:…>` link target directly after text on its line. A link target alone
/// at the start of a line is the link itself, and stays.
#[must_use]
pub fn lean(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(unified.len());
    let mut newlines = 0;
    for (index, line) in unified.split('\n').enumerate() {
        let line = without_link_targets(line);
        let line = line.trim_end();
        if index > 0 {
            newlines += 1;
        }
        if line.is_empty() {
            continue;
        }
        out.extend(std::iter::repeat_n('\n', newlines.min(2)));
        newlines = 0;
        out.push_str(line);
    }
    out.extend(std::iter::repeat_n('\n', newlines.min(2)));
    out
}

/// `line` without each link target that follows text, and the spaces before it.
fn without_link_targets(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let (before, from) = rest.split_at(open);
        match link_target_len(from) {
            Some(len) if !out.is_empty() || !before.trim().is_empty() => {
                out.push_str(before.trim_end_matches([' ', '\t']));
                rest = &from[len..];
            }
            _ => {
                out.push_str(before);
                out.push('<');
                rest = &from[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The length of the `<scheme:…>` at the start of `text`, when it is a closed web or mail link.
fn link_target_len(text: &str) -> Option<usize> {
    let inner = &text[1..];
    let lower = inner.get(..8).unwrap_or(inner).to_ascii_lowercase();
    if !["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return None;
    }
    let close = inner.find(|c: char| c == '>' || c.is_whitespace())?;
    (inner[close..].starts_with('>')).then_some(close + 2)
}

#[cfg(test)]
#[path = "lean_tests.rs"]
mod tests;
