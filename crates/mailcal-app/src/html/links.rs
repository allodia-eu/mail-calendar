//! Turning the addresses written as text in a sanitised body into links.
//!
//! Runs on the sanitiser's own output, which html5ever serialised: every `<` in text is `&lt;`,
//! every attribute value is double-quoted with its `"` escaped, and the only character references
//! in text are `&amp;`, `&lt;`, `&gt;` and `&nbsp;`. That regularity is what lets a scan find the
//! text between tags without a second parse. Text already inside an `<a>` is left alone, as is
//! the content of an element the serialiser writes raw (a `<style>` sheet above all).
//!
//! The anchors made here carry only an `href` the shared finder produced ([`crate::linkify`]),
//! whose scheme is on the same allowlist the sanitiser keeps, so the output is as inert as the
//! input.

use crate::linkify::find_links;

/// Elements whose content html5ever serialises unescaped. Only `<style>` survives the sanitiser;
/// the rest are here so the scan stays correct if the allowlist ever widens.
const RAW_TEXT: [&str; 8] = [
    "style",
    "script",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
    "noscript",
];

/// `html` with every address in its text wrapped in an `<a href>`.
pub(crate) fn linkify_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut anchors = 0usize;
    let mut cursor = 0;
    while cursor < html.len() {
        let Some(open) = html[cursor..].find('<').map(|at| cursor + at) else {
            push_text(&html[cursor..], anchors > 0, &mut out);
            break;
        };
        push_text(&html[cursor..open], anchors > 0, &mut out);
        let close = tag_end(html, open);
        let tag = &html[open..close];
        out.push_str(tag);
        cursor = close;
        let (name, closing) = tag_name(tag);
        if name.eq_ignore_ascii_case("a") {
            anchors = if closing {
                anchors.saturating_sub(1)
            } else {
                anchors + 1
            };
        } else if !closing && RAW_TEXT.iter().any(|raw| name.eq_ignore_ascii_case(raw)) {
            let end = raw_text_end(html, cursor, name);
            out.push_str(&html[cursor..end]);
            cursor = end;
        }
    }
    out
}

/// The index just past the `>` closing the tag that opens at `open`, skipping any `>` inside a
/// quoted attribute value. The end of the input when the tag never closes.
fn tag_end(html: &str, open: usize) -> usize {
    let mut quoted = false;
    for (at, c) in html[open..].char_indices() {
        match c {
            '"' => quoted = !quoted,
            '>' if !quoted => return open + at + 1,
            _ => {}
        }
    }
    html.len()
}

/// The tag's element name, and whether it is a closing tag. A comment or doctype has a name that
/// matches nothing this scan acts on.
fn tag_name(tag: &str) -> (&str, bool) {
    let inner = tag.trim_start_matches('<');
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let end = inner
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(inner.len());
    (&inner[..end], closing)
}

/// Where the raw content of `name`, starting at `from`, ends: at its closing tag.
fn raw_text_end(html: &str, from: usize, name: &str) -> usize {
    let needle = format!("</{}", name.to_ascii_lowercase());
    html[from..]
        .to_ascii_lowercase()
        .find(&needle)
        .map_or(html.len(), |at| from + at)
}

/// Copies a run of serialised text, wrapping each address in it unless it is already in a link.
fn push_text(escaped: &str, in_anchor: bool, out: &mut String) {
    if in_anchor || escaped.is_empty() {
        out.push_str(escaped);
        return;
    }
    let text = unescape(escaped);
    let links = find_links(&text);
    if links.is_empty() {
        out.push_str(escaped);
        return;
    }
    let mut cursor = 0;
    for link in links {
        escape_text(&text[cursor..link.range.start], out);
        out.push_str("<a href=\"");
        escape_attr(&link.href, out);
        // The `rel` ammonia gives every link it keeps, so a body sanitised twice (a quoted
        // original, at seed and on submit) comes out the same both times.
        out.push_str("\" rel=\"noopener noreferrer\">");
        escape_text(&text[link.range.clone()], out);
        out.push_str("</a>");
        cursor = link.range.end;
    }
    escape_text(&text[cursor..], out);
}

/// Reverses the four references html5ever writes in text. Anything else is left as written, and
/// is therefore copied back out unchanged.
fn unescape(escaped: &str) -> String {
    escaped
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", "\u{a0}")
        .replace("&amp;", "&")
}

fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
}

fn escape_attr(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
