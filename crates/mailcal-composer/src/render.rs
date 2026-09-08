use crate::{
    list::{List, ListKind},
    quote::{Quote, QuoteStyle},
    signature::Signature,
    types::{
        AttachmentDisposition, Block, ComposerDocument, ComposerOutput, DraftAttachment,
        InlineContent, InlineImage, OutputAttachment, TextRun,
    },
    validate::{ComposerResult, validate},
};

/// The indented quote's left border, inlined because mail clients drop `<head>`/`<style>`
/// (the same reason tables carry inline borders).
const INDENTED_QUOTE_STYLE: &str =
    "margin:0 0 0 0.8ex;border-left:2px solid #cccccc;padding-left:1ex";

/// The plain-text divider a line-and-header quote opens with.
const PLAIN_DIVIDER: &str = "________________________________";

/// The RFC 3676 §4.3 signature separator: a line of exactly `--` followed by a space. Readers and
/// list software key trailing-signature detection off this exact byte sequence, so the trailing
/// space is significant and must survive (it is why this is a constant rather than a literal
/// buried in the writer, where a formatter or a stray trim would silently eat it).
const PLAIN_SIGNATURE_DELIMITER: &str = "-- ";

/// The wrapper the signature's HTML is emitted inside. A class (not an id) because a forwarded
/// or quoted message can carry a second one, and duplicate ids are invalid; the name matches the
/// editor's `.allodia-signature` region so the block round-trips through the composer unchanged.
const SIGNATURE_OPEN: &str = "<div class=\"allodia-signature\">";

/// The opening of the line-and-header quote's HTML divider: a top border with `3pt` top
/// padding (not an `<hr>`), wrapping the header block. Closed with `</div></div>` after the
/// headers. The blue-grey (`rgb(181, 196, 223)`) and the border-not-rule shape are deliberate
/// interop: it is byte-for-byte what Outlook emits, so a reply threading back into an Outlook
/// mailbox is divided exactly where that reader expects.
const HEADER_DIVIDER_OPEN: &str =
    "<div style=\"padding:3pt 0 0;border-top:1pt solid rgb(181, 196, 223)\"><div>";

/// Validates and renders a compose document into send-ready body parts.
///
/// # Errors
/// Returns an error if the document fails validation; e.g. an inline run references an
/// attachment id that isn't present, or an attachment is declared inline without a content id.
pub fn render(document: &ComposerDocument) -> ComposerResult<ComposerOutput> {
    let validated = validate(document)?;
    let mut html = String::new();
    let mut plain_text = String::new();

    for (index, block) in document.blocks.iter().enumerate() {
        if index > 0 {
            plain_text.push('\n');
        }
        render_block_html(block, &mut html, &validated.attachments);
        render_block_text(block, &mut plain_text);
    }

    // Inline parts dedup by CID; regular attachments dedup by blob handle, and the two
    // pools are kept separate. The HTML points each inline `<img>` at its `cid:`, so every
    // referenced CID must have a matching inline part. Deduping inline parts by blob (in a
    // pool shared with the attachments) would drop a part whose blob also appears as a
    // regular attachment, or whose blob is reused by a second inline image under a different
    // CID; leaving the HTML with a dangling `cid:` and a broken inline image. Regular
    // attachments still dedup by blob so a file attached twice streams its bytes once (first
    // occurrence in document order wins).
    let mut inline_attachments = Vec::new();
    let mut attachments = Vec::new();
    let mut emitted_inline_cids = std::collections::HashSet::new();
    let mut seen_attachment_blobs = std::collections::HashSet::new();
    for attachment in &document.attachments {
        match &attachment.disposition {
            AttachmentDisposition::Inline { cid } => {
                if validated.referenced_inline.contains(&attachment.id)
                    && emitted_inline_cids.insert(cid)
                {
                    inline_attachments.push(output_attachment(attachment, Some(cid.clone())));
                }
            }
            // A regular attachment is always a host-staged file, so validation guarantees the
            // handle it dedups on.
            AttachmentDisposition::Attachment => {
                if let Some(blob) = &attachment.blob
                    && seen_attachment_blobs.insert(blob)
                {
                    attachments.push(output_attachment(attachment, None));
                }
            }
        }
    }

    Ok(ComposerOutput {
        html: wrap_document(&html),
        plain_text,
        inline_attachments,
        attachments,
    })
}

/// Wraps the rendered body fragment in a complete, standards-compliant HTML document.
/// Email clients commonly strip `<head>`/`<style>`, so the document carries only a
/// charset + viewport head and otherwise relies on the body's inline styles.
fn wrap_document(body: &str) -> String {
    let mut html = String::with_capacity(body.len() + 160);
    html.push_str(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body>",
    );
    html.push_str(body);
    html.push_str("</body></html>");
    html
}

fn render_block_html(
    block: &Block,
    out: &mut String,
    attachments: &std::collections::HashMap<&crate::types::AttachmentId, &DraftAttachment>,
) {
    match block {
        Block::Paragraph(paragraph) => {
            out.push_str("<p>");
            render_inlines_html(&paragraph.content, out, attachments);
            out.push_str("</p>");
        }
        Block::List(list) => render_list_html(list, out, attachments),
        Block::Quote(quote) => render_quote_html(quote, out),
        Block::Signature(signature) => render_signature_html(signature, out),
        Block::Table(table) => {
            // Email clients drop <head>/<style>, so table borders and padding must be
            // inline on every element or they render as a borderless run of cells.
            out.push_str("<table style=\"border-collapse:collapse\">");
            for row in &table.rows {
                out.push_str("<tr>");
                for cell in &row.cells {
                    out.push_str(
                        "<td style=\"border:1px solid #d8dde3;padding:6px;vertical-align:top\">",
                    );
                    render_inlines_html(&cell.content, out, attachments);
                    out.push_str("</td>");
                }
                out.push_str("</tr>");
            }
            out.push_str("</table>");
        }
    }
}

// Lists render with the browser's intrinsic `<ul>`/`<ol>` markers; unlike tables
// these need no inline style to survive mail clients stripping `<head>`/`<style>`.
// A sub-list is emitted INSIDE its parent `<li>` so nesting stays well-formed.
fn render_list_html(
    list: &List,
    out: &mut String,
    attachments: &std::collections::HashMap<&crate::types::AttachmentId, &DraftAttachment>,
) {
    let tag = list.kind.html_tag();
    out.push('<');
    out.push_str(tag);
    out.push('>');
    for item in &list.items {
        out.push_str("<li>");
        render_inlines_html(&item.content, out, attachments);
        if let Some(child) = &item.child {
            render_list_html(child, out, attachments);
        }
        out.push_str("</li>");
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

// A quote's `body_html` is emitted **verbatim**; it is an inert, pre-sanitised fragment the
// product core guarantees (at seed and again on submit; see `docs/composer-security.md`), so it
// is the one place the composer outputs HTML it did not itself construct from nodes. The indented
// style wraps it in a blockquote under the one-line attribution; the line-and-header style divides
// it off with a rule and a header block, at full width.
fn render_quote_html(quote: &Quote, out: &mut String) {
    match quote.style {
        QuoteStyle::Indented => {
            out.push_str("<p>");
            escape_html_text(&quote.attribution.line, out);
            out.push_str("</p><blockquote style=\"");
            out.push_str(INDENTED_QUOTE_STYLE);
            out.push_str("\">");
            out.push_str(&quote.body_html);
            out.push_str("</blockquote>");
        }
        QuoteStyle::LineAndHeader => {
            // The divider is a top border with a little top padding rather than an `<hr>`, then
            // a bold-labelled header block (`<b>From: </b>value`), the body following as a
            // sibling; matching what an Outlook reader renders for a quoted original.
            out.push_str(HEADER_DIVIDER_OPEN);
            for header in &quote.attribution.headers {
                out.push_str("<strong>");
                escape_html_text(&header.label, out);
                out.push_str(": </strong>");
                escape_html_text(&header.value, out);
                out.push_str("<br>");
            }
            out.push_str("</div></div><div>");
            out.push_str(&quote.body_html);
            out.push_str("</div>");
        }
    }
}

// A signature's `body_html` is emitted **verbatim**, for the same reason a quote's is: it is a raw
// fragment the core sanitises (`docs/composer-security.md`, Gate 10) rather than one the composer
// built from nodes. The wrapper is what the editor's `.allodia-signature` region round-trips, so
// the block a client hands back is the block that goes out.
fn render_signature_html(signature: &Signature, out: &mut String) {
    out.push_str(SIGNATURE_OPEN);
    out.push_str(&signature.body_html);
    out.push_str("</div>");
}

// The plain-text signature is delimited by RFC 3676's `-- ` line, which is how a reader knows
// where the message ends and the signature begins (and how a mailing list knows what to trim).
// The body's own text is used as-is: the composer never strips tags out of `body_html`.
fn render_signature_text(signature: &Signature, out: &mut String) {
    out.push_str(PLAIN_SIGNATURE_DELIMITER);
    for line in signature.body_plain.lines() {
        out.push('\n');
        out.push_str(line);
    }
}

fn render_inlines_html(
    inlines: &[InlineContent],
    out: &mut String,
    attachments: &std::collections::HashMap<&crate::types::AttachmentId, &DraftAttachment>,
) {
    for inline in inlines {
        match inline {
            InlineContent::Text(run) => render_run_html(run, out),
            InlineContent::Image(image) => render_image_html(image, out, attachments),
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

fn render_image_html(
    image: &InlineImage,
    out: &mut String,
    attachments: &std::collections::HashMap<&crate::types::AttachmentId, &DraftAttachment>,
) {
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
    out.push('>');
}

fn render_block_text(block: &Block, out: &mut String) {
    match block {
        Block::Paragraph(paragraph) => render_inlines_text(&paragraph.content, out),
        Block::List(list) => {
            let mut first = true;
            render_list_text(list, out, 0, &mut first);
        }
        Block::Quote(quote) => render_quote_text(quote, out),
        Block::Signature(signature) => render_signature_text(signature, out),
        Block::Table(table) => {
            for (row_index, row) in table.rows.iter().enumerate() {
                if row_index > 0 {
                    out.push('\n');
                }
                for (cell_index, cell) in row.cells.iter().enumerate() {
                    if cell_index > 0 {
                        out.push_str(" | ");
                    }
                    render_inlines_text(&cell.content, out);
                }
            }
        }
    }
}

// Plain-text lists: bullets render `- `, ordered items `1. `/`2. ` (numbering
// restarts per sub-list), each nesting level indented two spaces. `first` tracks
// whether any line has been written so the block never starts with a stray newline.
fn render_list_text(list: &List, out: &mut String, depth: usize, first: &mut bool) {
    for (index, item) in list.items.iter().enumerate() {
        if *first {
            *first = false;
        } else {
            out.push('\n');
        }
        for _ in 0..depth {
            out.push_str("  ");
        }
        match list.kind {
            ListKind::Bullet => out.push_str("- "),
            ListKind::Ordered => {
                out.push_str(&(index + 1).to_string());
                out.push_str(". ");
            }
        }
        render_inlines_text(&item.content, out);
        if let Some(child) = &item.child {
            render_list_text(child, out, depth + 1, first);
        }
    }
}

// The plain-text quote uses the original's `body_plain` (the composer never strips tags out of
// `body_html`). The indented style prefixes each quoted line with `> ` under the one-line
// attribution; the line-and-header style opens with a rule and the header block, then the body
// unprefixed at full width.
fn render_quote_text(quote: &Quote, out: &mut String) {
    match quote.style {
        QuoteStyle::Indented => {
            out.push_str(&quote.attribution.line);
            for line in quote.body_plain.lines() {
                out.push_str("\n> ");
                out.push_str(line);
            }
        }
        QuoteStyle::LineAndHeader => {
            out.push_str(PLAIN_DIVIDER);
            for header in &quote.attribution.headers {
                out.push('\n');
                out.push_str(&header.label);
                out.push_str(": ");
                out.push_str(&header.value);
            }
            for line in quote.body_plain.lines() {
                out.push('\n');
                out.push_str(line);
            }
        }
    }
}

fn render_inlines_text(inlines: &[InlineContent], out: &mut String) {
    for inline in inlines {
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
}

fn output_attachment(
    attachment: &DraftAttachment,
    cid: Option<crate::types::ContentId>,
) -> OutputAttachment {
    OutputAttachment {
        id: attachment.id.clone(),
        blob: attachment.blob.clone(),
        file_name: attachment.file_name.clone(),
        media_type: attachment.media_type.clone(),
        size: attachment.size,
        cid,
    }
}

fn escape_html_text(value: &str, out: &mut String) {
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

fn escape_html_attr(value: &str, out: &mut String) {
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
