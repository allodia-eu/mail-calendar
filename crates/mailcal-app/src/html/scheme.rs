//! A message drawn for the scheme the reader is looking at it in (`docs/reading-dark-mode.md`).
//!
//! In a dark render every colour the message sets is moved to its dark-page counterpart
//! ([`colour::darken`]): the `<style>` sheets, the `style` attributes, `bgcolor` and
//! `<font color>`. Images are never touched. In both renders a sender's
//! `@media (prefers-color-scheme: …)` rules are pinned to the scheme being drawn, so the web view's
//! own idea of the system's scheme, which differs per platform, never decides which of them apply.
//!
//! It runs on the **sanitised** fragment and changes only colour values and those media features,
//! so it cannot add markup, a URL or anything the CSP has to stop
//! (`docs/rendering-security.md`). The fragment it reads is the sanitiser's own serialisation,
//! whose shape is regular: every attribute value is double-quoted with `"` escaped, every `<` in
//! text is escaped, and `<style>` is the one element whose text is raw.

mod colour;
mod css;
mod named;

use std::borrow::Cow;

use colour::{Rgba, Role, darken};
use css::Edit;

/// Which page a message is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageScheme {
    /// The page the message was written for. Every print, and every read in a light app.
    #[default]
    Light,
    /// A dark page, with the message's colours moved to suit it.
    Dark,
}

/// `fragment` adapted to `scheme`. A light render of a message without colour-scheme queries is
/// the fragment itself.
pub(super) fn adapt(fragment: &str, scheme: MessageScheme) -> Cow<'_, str> {
    if scheme == MessageScheme::Light
        && find_ignoring_case(fragment, "prefers-color-scheme").is_none()
    {
        return Cow::Borrowed(fragment);
    }
    let edits = fragment_edits(fragment, scheme);
    if edits.is_empty() {
        Cow::Borrowed(fragment)
    } else {
        Cow::Owned(apply(fragment, &edits))
    }
}

/// Every edit in the fragment, in order.
fn fragment_edits(html: &str, scheme: MessageScheme) -> Vec<Edit> {
    let bytes = html.as_bytes();
    let mut edits = Vec::new();
    let mut i = 0;
    while let Some(offset) = html[i..].find('<') {
        let tag_start = i + offset;
        i = tag_start + 1;
        if !bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        let name_end = i + html[i..]
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(html.len() - i);
        let name = html[i..name_end].to_ascii_lowercase();
        i = attributes(html, name_end, &name, scheme, &mut edits);
        if name == "style" {
            let close = find_ignoring_case(&html[i..], "</style").map_or(html.len(), |at| i + at);
            edits.extend(
                css::sheet_edits(&html[i..close], scheme)
                    .into_iter()
                    .map(|edit| Edit {
                        start: edit.start + i,
                        end: edit.end + i,
                        text: edit.text,
                    }),
            );
            i = close;
        }
    }
    edits
}

/// Walks a start tag's attributes from `i`, recording the edits a dark render makes to them, and
/// returns where the tag ends.
fn attributes(
    html: &str,
    mut i: usize,
    tag: &str,
    scheme: MessageScheme,
    edits: &mut Vec<Edit>,
) -> usize {
    let bytes = html.as_bytes();
    loop {
        while bytes
            .get(i)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'/')
        {
            i += 1;
        }
        match bytes.get(i) {
            None => return i,
            Some(b'>') => return i + 1,
            Some(_) => {}
        }
        let name_start = i;
        while bytes
            .get(i)
            .is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b'=' | b'>' | b'/'))
        {
            i += 1;
        }
        if i == name_start {
            // A `=` with no name before it: step over it rather than stand on it.
            i += 1;
            continue;
        }
        let name = html[name_start..i].to_ascii_lowercase();
        if bytes.get(i) != Some(&b'=') || bytes.get(i + 1) != Some(&b'"') {
            continue;
        }
        let value_start = i + 2;
        let value_end = html[value_start..]
            .find('"')
            .map_or(html.len(), |at| value_start + at);
        // An unterminated value runs to the end of the fragment; never step past it.
        i = (value_end + 1).min(html.len());
        if scheme == MessageScheme::Dark {
            let value = &html[value_start..value_end];
            if let Some(text) = attribute(tag, &name, value) {
                edits.push(Edit {
                    start: value_start,
                    end: value_end,
                    text,
                });
            }
        }
    }
}

/// An attribute's value on a dark page, escaped as it was found, or `None` when it stays.
fn attribute(tag: &str, name: &str, escaped: &str) -> Option<String> {
    let role = match (tag, name) {
        (_, "style") => {
            let value = unescape(escaped);
            let edits = css::declaration_edits(&value);
            return (!edits.is_empty()).then(|| escape(&apply(&value, &edits)));
        }
        (_, "bgcolor") => Role::Fill,
        ("font", "color") => Role::Ink,
        _ => return None,
    };
    let value = unescape(escaped);
    let value = value.trim();
    let colour =
        Rgba::from_name(value).or_else(|| Rgba::from_hex(value.trim_start_matches('#')))?;
    darken(colour, role).map(Rgba::to_css)
}

/// The byte offset of the first ASCII-case-insensitive match of `needle`, which is ASCII.
fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// The fragment with every edit applied. Edits are in order and never overlap.
fn apply(text: &str, edits: &[Edit]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        if edit.start < at {
            continue;
        }
        out.push_str(&text[at..edit.start]);
        out.push_str(&edit.text);
        at = edit.end;
    }
    out.push_str(&text[at..]);
    out
}

/// The references the sanitiser's serialiser writes in an attribute value, decoded.
fn unescape(value: &str) -> Cow<'_, str> {
    if !value.contains('&') {
        return Cow::Borrowed(value);
    }
    Cow::Owned(
        value
            .replace("&quot;", "\"")
            .replace("&nbsp;", "\u{a0}")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

/// An attribute value escaped for a double-quoted attribute.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\u{a0}', "&nbsp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "scheme/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scheme/colour_tests.rs"]
mod colour_tests;
