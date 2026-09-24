//! The page a printed message is drawn on: the message's own header as labelled text above its
//! body, wrapped by [`render_document`] so it carries the reading view's CSP, base stylesheet and
//! remote-image choice unchanged (`docs/reading-actions.md`, "Printing a message").

use super::render_document;

/// One labelled line of a printed message's header, such as `From` and the sender.
///
/// The label is the client's, already localised; the core decides only how the lines are drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintHeaderLine {
    /// The line's name, as the client shows it (`From`, `Van`).
    pub label: String,
    /// What the message says for it. A line whose value is empty is left out.
    pub value: String,
}

/// The printed header's stylesheet. Class selectors throughout, so a message's own rule for a
/// bare `div` or `h1` does not outrank it, and `body` loses the reading view's inset because the
/// printer's margins are the page's.
const PRINT_CSS: &str = "@media print{body{padding:0}}\
    .mailcal-print-header{margin:0 0 12px;padding:0 0 10px;border-bottom:1px solid #c8c8c8;\
    font-size:13px;line-height:1.4}\
    .mailcal-print-subject{font-size:18px;font-weight:600;margin:0 0 8px}\
    .mailcal-print-line{margin:0}\
    .mailcal-print-label{font-weight:600}\
    .mailcal-print-plain{white-space:pre-wrap}";

/// Builds the document a client prints for one message.
///
/// `html` is the reading snapshot's **sanitised** fragment and is used as is; `plain` is the
/// plain-text body, escaped here and kept to its own line breaks. With neither, the page is the
/// header alone. Every header value is escaped, so a subject or an address is always text and
/// never markup. `load_remote_images` is the reader's choice for this message, so printing never
/// loads what reading did not.
#[must_use]
pub fn render_print_document(
    subject: &str,
    lines: &[PrintHeaderLine],
    html: Option<&str>,
    plain: Option<&str>,
    load_remote_images: bool,
) -> String {
    let mut fragment = format!("<style>{PRINT_CSS}</style><div class=\"mailcal-print-header\">");
    fragment.push_str("<div class=\"mailcal-print-subject\">");
    escape_text(subject, &mut fragment);
    fragment.push_str("</div>");
    for line in lines.iter().filter(|line| !line.value.is_empty()) {
        fragment.push_str("<div class=\"mailcal-print-line\"><span class=\"mailcal-print-label\">");
        escape_text(&line.label, &mut fragment);
        fragment.push_str(":</span> ");
        escape_text(&line.value, &mut fragment);
        fragment.push_str("</div>");
    }
    fragment.push_str("</div>");
    if let Some(html) = html {
        fragment.push_str(html);
    } else if let Some(plain) = plain {
        fragment.push_str("<div class=\"mailcal-print-plain\">");
        escape_text(plain, &mut fragment);
        fragment.push_str("</div>");
    }
    render_document(&fragment, load_remote_images)
}

fn escape_text(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
#[path = "print_tests.rs"]
mod print_tests;
