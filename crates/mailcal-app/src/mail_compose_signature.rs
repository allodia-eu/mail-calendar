//! Send-time hardening for the signature block: re-sanitise it, and turn its inline `data:`
//! images into `cid:` MIME parts.
//!
//! Both run on the rich-draft path, right before the document is rendered
//! ([`crate::mail_compose`]). They are the signature's half of two rules that already govern a
//! quoted original:
//!
//! - **Sanitise on submit** (`docs/composer-security.md`, Gate 10). The signature body is authored
//!   by the user and then round-tripped through the host's WebView editor, so what comes back is
//!   untrusted; exactly like a quote. Running this in the shared use-case layer means the gate
//!   holds on every platform whatever a client's editor emits.
//! - **`data:` → `cid:` on the way out.** A signature stores its images inline as `data:` URIs,
//!   which is right for the library (one self-contained file) and wrong for the wire: Outlook's
//!   reader blocks `data:` images and renders `cid:` ones, so a logo sent as `data:` is an empty
//!   box for a large share of recipients. This is the same destination as [`crate::mail_compose`]'s
//!   quote rewrite but the opposite starting point: a quote's images *were* MIME parts and keep
//!   their original `Content-ID`s, while a signature's bytes have never been a part, so an id is
//!   minted here.
//!
//! A rewrite that cannot be done safely (an unparseable `data:` URI, a non-image media type,
//! invalid base64) leaves the image untouched rather than dropping it: a `data:` image renders in
//! most readers, and losing the user's logo is worse than an interoperability shortfall.

use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use engine_api::{ContentIdHeader, DraftAttachment};
use mailcal_composer::{Block, ComposerDocument};

use crate::{helpers::generated_content_id, html::image_media_type};

/// The attribute prefix an inline image is matched on. Anchoring on the whole `src="…"` value
/// (rather than the bare `data:` substring) keeps the rewrite inside one attribute; it can never
/// splice the middle of another attribute's value.
const SRC_PREFIX: &str = "src=\"data:";

/// Re-sanitises every signature body to the inert, safe subset (`crate::html::sanitize`) in place
/// : the Gate-10 treatment a quoted original already gets, applied to the other block the composer
/// emits verbatim.
pub(super) fn sanitize_signature_bodies(document: &mut ComposerDocument) {
    for block in &mut document.blocks {
        if let Block::Signature(signature) = block {
            signature.body_html = crate::html::sanitize(&signature.body_html).html;
        }
    }
}

/// Rewrites each signature's inline `data:` images to `cid:` references and returns one inline
/// [`DraftAttachment`] per distinct image, so the sent message carries them as `multipart/related`
/// parts. Identical images (the same `data:` URI, in one signature or across two) share one part.
///
/// Runs **after** [`sanitize_signature_bodies`]: the sanitiser preserves `data:` images, and
/// rewriting first would hand it `cid:` references to parts it knows nothing about.
pub(super) fn reattach_signature_cids(document: &mut ComposerDocument) -> Vec<DraftAttachment> {
    let mut attachments = Vec::new();
    // Keyed on the whole `data:` URI, so the same logo in a signature used twice in one document
    // (a reply that also quotes an earlier one) attaches its bytes once.
    let mut minted: HashMap<String, String> = HashMap::new();
    for block in &mut document.blocks {
        if let Block::Signature(signature) = block {
            signature.body_html =
                rewrite_data_images(&signature.body_html, &mut minted, &mut attachments);
        }
    }
    attachments
}

/// Walks `html` for `src="data:…"` attributes, replacing each safely-decodable image with a
/// `cid:` reference and appending its part. Anything it cannot decode is copied through unchanged.
fn rewrite_data_images(
    html: &str,
    minted: &mut HashMap<String, String>,
    attachments: &mut Vec<DraftAttachment>,
) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(SRC_PREFIX) {
        let (before, from_match) = rest.split_at(start);
        out.push_str(before);
        let value_start = SRC_PREFIX.len() - "data:".len();
        // The attribute value runs to the next double quote. A `data:` URI contains none.
        let Some(end) = from_match[value_start..]
            .find('"')
            .map(|at| at + value_start)
        else {
            // Unterminated attribute, nothing left to scan, so emit the remainder verbatim.
            out.push_str(from_match);
            return out;
        };
        let uri = &from_match[value_start..end];
        match cid_for(uri, minted, attachments) {
            Some(cid) => {
                out.push_str("src=\"cid:");
                escape_attr_value(&cid, &mut out);
                out.push('"');
            }
            // Not something we can safely turn into a part; keep the `data:` image as it was.
            None => out.push_str(&from_match[..=end]),
        }
        rest = &from_match[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The `Content-ID` for one `data:` URI; reusing the id already minted for identical bytes, or
/// decoding and minting a new one (appending its part). `None` when the URI is not a base64
/// `image/*` we can decode, or when the minted id fails header validation.
fn cid_for(
    uri: &str,
    minted: &mut HashMap<String, String>,
    attachments: &mut Vec<DraftAttachment>,
) -> Option<String> {
    if let Some(existing) = minted.get(uri) {
        return Some(existing.clone());
    }
    let (media, bytes) = decode_data_image(uri)?;
    // The sequence is the count of parts so far, so several images in one signature; rewritten
    // inside a single clock tick; cannot collide on the wall-clock component alone.
    let id = generated_content_id(attachments.len());
    let content_id = ContentIdHeader::new(&id).ok()?;
    attachments.push(DraftAttachment::inline(
        file_name(&media, attachments.len()),
        media,
        content_id,
        bytes,
    ));
    minted.insert(uri.to_owned(), id.clone());
    Some(id)
}

/// Decodes a `data:image/<subtype>;base64,<payload>` URI into its media type and bytes.
///
/// Deliberately narrow: only base64 payloads (never percent-encoded ones) and only `image/*`
/// media types that pass [`image_media_type`]'s restricted-name check, so the rewrite can never
/// attach an executable document under a `cid:` an `<img>` points at. Anything else returns
/// `None` and the image is left as it was.
///
/// Shared with [`crate::mail_compose`], which decodes the same shape for a pasted or dropped
/// picture the editor carried in the document rather than behind a host blob handle.
pub(super) fn decode_data_image(uri: &str) -> Option<(String, Vec<u8>)> {
    let body = uri.strip_prefix("data:")?;
    let (media, payload) = body.split_once(";base64,")?;
    let media = image_media_type(media)?;
    // Strict decoding: a payload the encoder would never have produced (bad padding, stray
    // characters) is a signal we do not understand this URI, not something to guess at.
    let bytes = STANDARD.decode(payload).ok()?;
    (!bytes.is_empty()).then_some((media, bytes))
}

/// A display file name for a minted inline part, from its media type. Cosmetic only: the part is
/// addressed by its `Content-ID`, not its name, but a reader that lists parts should not show a
/// row of blanks.
fn file_name(media: &str, seq: usize) -> String {
    let ext = match media.rsplit('/').next() {
        Some("jpeg") => "jpg",
        Some("svg+xml") => "svg",
        Some(sub) if !sub.is_empty() && sub.bytes().all(|b| b.is_ascii_alphanumeric()) => sub,
        _ => "img",
    };
    format!("signature-{seq}.{ext}")
}

/// Escapes the characters significant inside a double-quoted attribute value, so a minted
/// `Content-ID` can never break out of the `src="cid:…"` it is placed in. Our own ids contain
/// none of these; this is defence in depth, matching `html::restore_cid_images`.
fn escape_attr_value(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use engine_api::ContentIdHeader;
    use mailcal_composer::{Block, ComposerDocument, Paragraph, Signature};

    use super::{reattach_signature_cids, sanitize_signature_bodies};

    /// A 1×1 transparent PNG, base64; small enough to inline in a test, real enough to decode.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    fn document(body_html: &str) -> ComposerDocument {
        ComposerDocument {
            blocks: vec![
                Block::Paragraph(Paragraph {
                    content: Vec::new(),
                }),
                Block::Signature(Signature {
                    body_html: body_html.to_owned(),
                    body_plain: "Alice".to_owned(),
                }),
            ],
            attachments: Vec::new(),
        }
    }

    fn signature_html(document: &ComposerDocument) -> &str {
        match &document.blocks[1] {
            Block::Signature(signature) => &signature.body_html,
            other => panic!("expected a signature block, got {other:?}"),
        }
    }

    #[test]
    fn an_inline_data_image_becomes_a_cid_part() {
        // The interop rule: Outlook's reader blocks `data:` images and renders `cid:` ones, so a
        // signature logo has to leave as a MIME part or a large share of recipients see a gap.
        let mut doc = document(&format!(
            "<p>Alice</p><img src=\"data:image/png;base64,{PNG}\" alt=\"logo\">"
        ));
        let attachments = reattach_signature_cids(&mut doc);

        assert_eq!(attachments.len(), 1);
        let html = signature_html(&doc);
        assert!(!html.contains("data:image"), "{html}");
        let cid = html
            .split("src=\"cid:")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("a cid reference");
        // The reference and the part must name the same id, or the reader shows a broken image.
        assert_eq!(
            attachments[0].content_id().map(ContentIdHeader::as_str),
            Some(cid)
        );
        assert_eq!(attachments[0].media_type, "image/png");
        assert!(!attachments[0].content.is_empty());
    }

    #[test]
    fn the_same_image_twice_attaches_its_bytes_once() {
        // A logo above and below a divider is one part with two references; otherwise every
        // repeat re-sends the bytes.
        let mut doc = document(&format!(
            "<img src=\"data:image/png;base64,{PNG}\"><hr><img src=\"data:image/png;base64,{PNG}\">"
        ));
        let attachments = reattach_signature_cids(&mut doc);

        assert_eq!(attachments.len(), 1);
        let html = signature_html(&doc);
        assert_eq!(html.matches("src=\"cid:").count(), 2);
    }

    #[test]
    fn an_undecodable_or_non_image_data_uri_is_left_alone() {
        // Losing the user's logo is worse than a `data:` image an Outlook reader hides, so a URI
        // we cannot safely turn into a part is copied through untouched. The `text/html` case is
        // the one that matters: it must never become a `cid:` part an `<img>` points at.
        let mut doc = document(
            "<img src=\"data:image/png;base64,!!!not-base64!!!\">\
             <img src=\"data:text/html;base64,PHNjcmlwdD4=\">",
        );
        let attachments = reattach_signature_cids(&mut doc);

        assert!(attachments.is_empty());
        let html = signature_html(&doc);
        assert_eq!(html.matches("data:").count(), 2);
        assert!(!html.contains("cid:"));
    }

    #[test]
    fn surrounding_markup_survives_the_rewrite() {
        // The scanner walks attribute by attribute; everything between two images has to come
        // through byte-for-byte, including an unrelated `src` and text that mentions `data:`.
        let mut doc = document(&format!(
            "<p>before</p><img src=\"data:image/png;base64,{PNG}\" width=\"20\">\
             <a href=\"https://x.test\">link</a><p>after data: not an image</p>"
        ));
        reattach_signature_cids(&mut doc);

        let html = signature_html(&doc);
        assert!(html.starts_with("<p>before</p>"), "{html}");
        assert!(html.contains("width=\"20\""), "{html}");
        assert!(
            html.contains("<a href=\"https://x.test\">link</a>"),
            "{html}"
        );
        assert!(html.ends_with("<p>after data: not an image</p>"), "{html}");
    }

    #[test]
    fn the_signature_body_is_re_sanitized_on_submit() {
        // Gate 10 for signatures: the body is authored by the user, then round-tripped through
        // the host's editor, so script and handlers are stripped here rather than trusted away.
        let mut doc = document(
            "<p onclick=\"steal()\">Alice</p><script>alert(1)</script>\
             <img src=\"data:image/png;base64,AAAA\">",
        );
        sanitize_signature_bodies(&mut doc);

        let html = signature_html(&doc);
        assert!(!html.contains("script"), "{html}");
        assert!(!html.contains("onclick"), "{html}");
        // …while a `data:` image survives it, which is what makes the rewrite below possible.
        assert!(html.contains("data:image/png;base64,AAAA"), "{html}");
    }

    #[test]
    fn a_signature_free_document_is_untouched() {
        let mut doc = ComposerDocument {
            blocks: vec![Block::Paragraph(Paragraph {
                content: Vec::new(),
            })],
            attachments: Vec::new(),
        };
        let before = doc.clone();
        sanitize_signature_bodies(&mut doc);
        assert!(reattach_signature_cids(&mut doc).is_empty());
        assert_eq!(doc, before);
    }
}
