//! Turning a message's sanitised HTML into plain text.
//!
//! # Why the core does this, and not the adapter
//!
//! A message body is attacker-authored text that ends up in a language model's context. That
//! cannot be fixed, only bounded, and the cheapest large bound available is: **never hand over
//! HTML**. HTML is a strictly larger injection surface than text: a hidden `<span>`,
//! white-on-white text, `display:none`, CSS `content`, and *none* of that is removed by
//! sanitisation, because sanitisation is about stopping script execution in a WebView, not about
//! what a model reads.
//!
//! So the conversion happens here, below the adapter, and `MessageDetail` carries only text. An
//! adapter over that type structurally cannot leak HTML it was never given, which is a stronger
//! guarantee than a rule in a policy module that a future contributor can route around.
//!
//! This runs on **already-sanitised** HTML (`crate::html::sanitize`), so it faces an inert
//! subset rather than arbitrary markup. It is still written defensively: unterminated tags,
//! nested comments and stray `<` are all treated as text-or-drop, never as a parse failure.

/// Extracts readable plain text from sanitised message HTML.
///
/// Block-level boundaries become newlines so paragraphs and list items stay apart (a wall of
/// run-together text reads as one sentence to a model as much as to a person), consecutive blank
/// lines collapse, and `<script>`/`<style>` content is dropped rather than emitted as prose.
pub(super) fn to_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        push_text(&mut out, &rest[..open]);
        let after = &rest[open + 1..];
        // A comment runs to `-->`; anything unterminated swallows the remainder, which is the
        // safe direction (drop, never emit markup as prose).
        if let Some(body) = after.strip_prefix("!--") {
            rest = body.split_once("-->").map_or("", |(_, tail)| tail);
            continue;
        }
        let Some((tag, tail)) = after.split_once('>') else {
            // An unterminated `<` is literal text, not a tag. It is also the end of the input,
            // so consume the remainder here rather than letting the tail append it twice.
            push_text(&mut out, &rest[open..]);
            return out.trim().to_owned();
        };
        let name = tag_name(tag);
        if matches!(name.as_str(), "script" | "style") {
            // Drop the element's content wholesale.
            let close = format!("</{name}");
            rest = tail
                .split_once(close.as_str())
                .and_then(|(_, after_close)| after_close.split_once('>'))
                .map_or("", |(_, tail)| tail);
            continue;
        }
        if is_break(&name) {
            push_break(&mut out);
        }
        rest = tail;
    }
    push_text(&mut out, rest);
    out.trim().to_owned()
}

/// The lowercased element name of a tag's inner text (`"/p class=x"` → `"p"`).
fn tag_name(tag: &str) -> String {
    tag.trim_start_matches('/')
        .trim_start()
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Whether an element boundary should read as a line break. Deliberately a small, explicit set:
/// the sanitiser's allowed subset is small, and treating every unknown tag as a break would turn
/// inline styling into shredded text.
fn is_break(name: &str) -> bool {
    matches!(
        name,
        "br" | "p"
            | "div"
            | "li"
            | "ul"
            | "ol"
            | "tr"
            | "table"
            | "blockquote"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "pre"
    )
}

/// Appends a single newline, collapsing runs so a `</p><div>` pair is one break, not two.
fn push_break(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

/// Appends a run of text with its entities decoded and its whitespace collapsed.
fn push_text(out: &mut String, text: &str) {
    for ch in decode_entities(text).chars() {
        if ch == '\n' {
            push_break(out);
        } else if ch.is_whitespace() {
            if !out.ends_with(' ') && !out.ends_with('\n') && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
}

/// Decodes the HTML entities that survive sanitisation: the five named ones the sanitiser emits,
/// a non-breaking space, and numeric character references. An unrecognized entity is left as
/// written; showing `&foo;` is better than silently deleting text a reader needs.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        // An entity is short; a `&` with no `;` within a few characters is a literal ampersand.
        if let Some(end) = after.find(';').filter(|end| *end <= 8) {
            if let Some(ch) = decode_one(&after[..end]) {
                out.push(ch);
            } else {
                out.push('&');
                out.push_str(&after[..end]);
                out.push(';');
            }
            rest = &after[end + 1..];
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// One entity body (between `&` and `;`) as a character, or `None` if unrecognized.
fn decode_one(body: &str) -> Option<char> {
    match body {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let digits = body.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::to_plain;

    #[test]
    fn block_boundaries_become_line_breaks_and_inline_markup_does_not() {
        assert_eq!(
            to_plain("<p>Hello <b>there</b></p><p>Second line</p>"),
            "Hello there\nSecond line",
        );
    }

    #[test]
    fn script_and_style_content_is_dropped_not_read_aloud() {
        // The sanitiser already strips these from a rendered body, but this runs on whatever it
        // is handed, and a model reading CSS as prose is exactly the injection surface the
        // plain-text rule exists to shrink.
        assert_eq!(
            to_plain("<style>.a{content:'ignore me'}</style><p>Real text</p>"),
            "Real text",
        );
        assert_eq!(
            to_plain("<script>alert('do this instead')</script>Body"),
            "Body",
        );
    }

    #[test]
    fn entities_are_decoded_and_unknown_ones_are_left_alone() {
        assert_eq!(to_plain("A &amp; B &lt;c&gt; &#39;d&#39;"), "A & B <c> 'd'");
        assert_eq!(to_plain("100 &euro; today"), "100 &euro; today");
        // A semicolon-less `&amp` is left alone. A browser would render it as `&`; showing the
        // four extra characters is the cheaper mistake than a decoder that guesses where an
        // entity ends in prose that legitimately contains ampersands.
        assert_eq!(to_plain("Tom &amp Jerry"), "Tom &amp Jerry");
    }

    #[test]
    fn a_stray_angle_bracket_is_text_rather_than_a_swallowed_body() {
        // Losing the rest of a message to one unterminated `<` would be a silent, total data
        // loss in the one place a caller cannot notice it.
        assert_eq!(to_plain("5 < 6 and that is that"), "5 < 6 and that is that");
        assert_eq!(
            to_plain("<p>Kept</p><b unterminated"),
            "Kept\n<b unterminated"
        );
    }

    #[test]
    fn whitespace_collapses_the_way_a_renderer_would() {
        assert_eq!(
            to_plain("<div>  lots\n\n   of   space  </div>"),
            "lots\nof space",
        );
    }
}
