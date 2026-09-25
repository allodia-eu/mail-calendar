//! Inline content (text runs, links and pictures) rendered into the outgoing HTML and plain text.

use std::collections::HashMap;

use crate::{
    link::LinkUrl,
    types::{
        AttachmentDisposition, AttachmentId, DraftAttachment, InlineContent, InlineImage, TextRun,
    },
};

type Attachments<'a> = HashMap<&'a AttachmentId, &'a DraftAttachment>;

/// The link an inline item belongs to: a picture never carries one.
fn link_of(inline: &InlineContent) -> Option<&LinkUrl> {
    match inline {
        InlineContent::Text(run) => run.link.as_ref(),
        InlineContent::Image(_) => None,
    }
}

/// Consecutive inline items sharing a link target, so a link whose words carry different marks
/// ("the **new** schedule") is one `<a>` rather than three, and one `<url>` in the plain text.
fn link_groups(
    inlines: &[InlineContent],
) -> impl Iterator<Item = (Option<&LinkUrl>, &[InlineContent])> {
    let mut rest = inlines;
    std::iter::from_fn(move || {
        let first = rest.first()?;
        let link = link_of(first);
        let len = rest
            .iter()
            .position(|inline| link_of(inline) != link)
            .unwrap_or(rest.len());
        let (group, tail) = rest.split_at(len);
        rest = tail;
        Some((link, group))
    })
}

pub(crate) fn render_inlines_html(
    inlines: &[InlineContent],
    out: &mut String,
    attachments: &Attachments<'_>,
) {
    for (link, group) in link_groups(inlines) {
        if let Some(link) = link {
            out.push_str("<a href=\"");
            escape_html_attr(link.as_str(), out);
            out.push_str("\">");
        }
        for inline in group {
            match inline {
                InlineContent::Text(run) => render_run_html(run, out),
                InlineContent::Image(image) => render_image_html(image, out, attachments),
            }
        }
        if link.is_some() {
            out.push_str("</a>");
        }
    }
}

/// The inline CSS a run's non-structural marks need, as one declaration list.
///
/// One span carries all of them rather than one span each: a run that is large, red and highlighted
/// would otherwise ship three nested wrappers, and mail clients that rewrite or flatten CSS are
/// more likely to lose a mark the deeper it is nested. Empty when the run has none, which is the
/// common case and emits no span at all. Every value is machine-produced: a `u8` of pixels, or a
/// [`TextColor`](crate::TextColor) validated to `#rrggbb`: so none of it needs escaping.
fn run_style(run: &TextRun) -> String {
    let mut style = String::new();
    let mut push = |property: &str, value: &str| {
        if !style.is_empty() {
            style.push(';');
        }
        style.push_str(property);
        style.push(':');
        style.push_str(value);
    };
    if let Some(size) = run.font_size {
        push("font-size", &format!("{}px", size.css_px()));
    }
    if let Some(color) = &run.color {
        push("color", color.as_str());
    }
    if let Some(highlight) = &run.highlight {
        push("background-color", highlight.as_str());
    }
    style
}

fn render_run_html(run: &TextRun, out: &mut String) {
    let style = run_style(run);
    let opened_span = !style.is_empty();
    if opened_span {
        out.push_str("<span style=\"");
        out.push_str(&style);
        out.push_str("\">");
    }
    if run.bold {
        out.push_str("<strong>");
    }
    if run.italic {
        out.push_str("<em>");
    }
    if run.underline {
        out.push_str("<u>");
    }
    escape_html_text(&run.text, out);
    if run.underline {
        out.push_str("</u>");
    }
    if run.italic {
        out.push_str("</em>");
    }
    if run.bold {
        out.push_str("</strong>");
    }
    if opened_span {
        out.push_str("</span>");
    }
}

fn render_image_html(image: &InlineImage, out: &mut String, attachments: &Attachments<'_>) {
    let Some(attachment) = attachments.get(&image.attachment_id) else {
        return;
    };
    let AttachmentDisposition::Inline { cid } = &attachment.disposition else {
        return;
    };
    out.push_str("<img src=\"cid:");
    escape_html_attr(cid.as_str(), out);
    out.push_str("\" alt=\"");
    escape_html_attr(&image.alt_text, out);
    out.push('"');
    if let Some(width) = image.width_px {
        out.push_str(" width=\"");
        out.push_str(&width.to_string());
        out.push('"');
    }
    // The size is stated twice, and neither statement is redundant.
    //
    // The `width` **attribute** is a presentational hint, which the cascade places below every
    // author stylesheet rule: a recipient whose client carries so much as an `img { width: … }`
    // rule silently overrides it, and the reader sees a size the sender never chose. The inline
    // **style** is an author declaration and loses only to `!important`, so it is the one that
    // actually holds. The attribute stays for the older Word-based Outlook, which reads it and
    // not the style, which is also why this renderer inlines every other style it emits.
    //
    // `max-width` is here whether or not a width is, and the picture nobody resized is the case
    // it matters for: with no width to state it renders at its intrinsic size, and a photograph
    // off a phone is thousands of pixels wide. Our own reading view caps every image, so a
    // message that overflowed a narrower client would look correct to us.
    //
    // No `height`: the document carries a width alone, so the height is left to follow the
    // picture's own ratio rather than being guessed at from bytes this crate never decodes.
    out.push_str(" style=\"");
    if let Some(width) = image.width_px {
        out.push_str("width: ");
        out.push_str(&width.to_string());
        out.push_str("px; ");
    }
    out.push_str("max-width: 100%\">");
}

/// A link in the plain-text part is its words followed by `<target>`, the form RFC 3986 appendix C
/// suggests for a URL in running text. When the words already are the target, which is what an
/// address the user typed becomes, it is written once.
pub(crate) fn render_inlines_text(inlines: &[InlineContent], out: &mut String) {
    for (link, group) in link_groups(inlines) {
        let start = out.len();
        for inline in group {
            match inline {
                InlineContent::Text(run) => out.push_str(&run.text),
                InlineContent::Image(image) => {
                    out.push('[');
                    out.push_str(if image.alt_text.trim().is_empty() {
                        "image"
                    } else {
                        image.alt_text.trim()
                    });
                    out.push(']');
                }
            }
        }
        if let Some(link) = link {
            let words = out[start..].trim();
            let target = link.as_str();
            let bare = target.strip_prefix("mailto:").unwrap_or(target);
            if words != target && words != bare {
                out.push_str(" <");
                out.push_str(target);
                out.push('>');
            }
        }
    }
}

pub(crate) fn escape_html_text(value: &str, out: &mut String) {
    // Element-text context: only these three are significant. Quotes need no escaping here
    // (attribute values go through `escape_html_attr`).
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

pub(crate) fn escape_html_attr(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}
