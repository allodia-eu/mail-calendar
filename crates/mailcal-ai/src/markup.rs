//! A drafted reply's formatting, read the way the composer draws it. The model may use a small
//! Markdown subset (`docs/ai.md`, "Drafting a reply"), which the editor bundle builds as elements;
//! what the person then sends is compared, as plain text, with the draft as plain text.

/// The draft as the composer's plain text renders it: bold and italic markers taken off, every
/// bulleted item as `- `, every numbered item as `N. `, a heading without its hashes. Anything the
/// editor would leave as text is left as written.
#[must_use]
pub fn draft_plain(text: &str) -> String {
    text.split('\n')
        .map(plain_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn plain_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if let Some(rest) = heading(trimmed) {
        return inline(rest);
    }
    if let Some(rest) = ["- ", "* ", "• "]
        .iter()
        .find_map(|marker| trimmed.strip_prefix(marker))
    {
        return format!("- {}", inline(rest.trim_start()));
    }
    if let Some((number, rest)) = numbered(trimmed) {
        return format!("{number}. {}", inline(rest));
    }
    inline(line)
}

fn heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

fn numbered(line: &str) -> Option<(&str, &str)> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if !(1..=3).contains(&digits) {
        return None;
    }
    let rest = line[digits..].strip_prefix(['.', ')'])?;
    rest.starts_with(char::is_whitespace)
        .then(|| (&line[..digits], rest.trim_start()))
}

/// Takes off each `**…**` and `*…*` pair whose inner text starts and ends with a non-space, the
/// first closing marker winning, as the editor's pattern does.
fn inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('*') {
        out.push_str(&rest[..at]);
        let from = &rest[at..];
        if let Some((inner, taken)) = pair(from) {
            out.push_str(inner);
            rest = &from[taken..];
        } else {
            out.push('*');
            rest = &from[1..];
        }
    }
    out.push_str(rest);
    out
}

/// A `**…**` pair, else a `*…*` pair, at the start of `from`: its inner text, and how much of
/// `from` it takes.
fn pair(from: &str) -> Option<(&str, usize)> {
    if let Some(inner) = from
        .strip_prefix("**")
        .and_then(|after| closed(after, "**"))
    {
        return Some((inner, inner.len() + 4));
    }
    closed(&from[1..], "*").map(|inner| (inner, inner.len() + 2))
}

/// The inner text up to the first `marker` that closes a span starting and ending with a
/// non-space.
fn closed<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    if text.is_empty() || text.starts_with(char::is_whitespace) {
        return None;
    }
    let mut from = 0;
    while let Some(offset) = text[from..].find(marker) {
        let end = from + offset;
        if end > 0 && !text[..end].ends_with(char::is_whitespace) {
            return Some(&text[..end]);
        }
        from = end + 1;
    }
    None
}

#[cfg(test)]
#[path = "markup_tests.rs"]
mod tests;
