//! Send-time hardening for a quoted original: re-sanitise it, and turn the reading view's
//! resolved inline `data:` images back into `cid:` parts.
//!
//! Both run on the rich-draft path right before render ([`crate::mail_compose`]), and both mirror
//! what [`crate::mail_compose_signature`] does to the other raw-HTML block. The difference is
//! where the bytes came from: a quote's images **were** MIME parts on the inbound message, so they
//! keep their original `Content-ID`s, while a signature's have never been parts and get minted
//! ones. Keeping the original ids is what an Outlook reader and a long-thread spam filter both
//! expect (`docs/composer-security.md`, Gate 10).

use std::collections::HashSet;

use engine_api::{ContentIdHeader, DraftAttachment, InlinePart};
use mailcal_composer::{Block, ComposerDocument};

/// Re-sanitises every quoted original's HTML to the inert, safe subset (`crate::html::sanitize`)
/// in place. Called on the rich-draft path right before render: the quote body is HTML a host's
/// WebView editor handed back, so it is re-hardened here rather than trusted: the same sanitizer
/// the reading view runs on inbound mail, applied now to outbound quoted content.
pub(super) fn sanitize_quote_bodies(document: &mut ComposerDocument) {
    for block in &mut document.blocks {
        if let Block::Quote(quote) = block {
            quote.body_html = crate::html::sanitize(&quote.body_html).html;
        }
    }
}

/// Whether any quote block carries a resolved inline `data:` image (what the reading view
/// produces from the original's `cid:` parts) worth re-attaching, so the inline-parts fetch (a
/// raw-source blob read + MIME parse, or a provider round-trip) is skipped for the common
/// reply/forward whose quoted original had no inline images.
pub(super) fn quotes_reference_inline_images(document: &ComposerDocument) -> bool {
    document
        .blocks
        .iter()
        .any(|block| matches!(block, Block::Quote(quote) if quote.body_html.contains("data:image")))
}

/// Rewrites each quoted original's inline `data:` images back to `cid:` references to their
/// original parts (`crate::html::restore_cid_images`), returning one inline [`DraftAttachment`]
/// per distinct part so the sent message carries them as `multipart/related` `cid:` parts with
/// their **original** `Content-ID`s. A no-op when `inline_parts` is empty (a new message, or a
/// reply/forward whose original had no inline images).
pub(super) fn reattach_quote_cids(
    document: &mut ComposerDocument,
    inline_parts: &[InlinePart],
) -> Vec<DraftAttachment> {
    // Only parts whose `Content-ID` survives header validation can be re-attached; rewriting a
    // `data:` image to `cid:` for an unattachable id would leave a dangling reference, so those
    // parts are excluded up front and keep their (still-rendering) `data:` image untouched.
    let attachable: Vec<&InlinePart> = inline_parts
        .iter()
        .filter(|part| ContentIdHeader::new(part.content_id()).is_ok())
        .collect();
    let mut attachments = Vec::new();
    let mut attached: HashSet<&str> = HashSet::new();
    for block in &mut document.blocks {
        if let Block::Quote(quote) = block {
            let (restored, matched) =
                crate::html::restore_cid_images(&quote.body_html, &attachable);
            quote.body_html = restored;
            for part in matched {
                // One part may be referenced by more than one quote block; attach it once.
                if !attached.insert(part.content_id()) {
                    continue;
                }
                let Ok(content_id) = ContentIdHeader::new(part.content_id()) else {
                    continue;
                };
                attachments.push(DraftAttachment::inline(
                    inline_file_name(part),
                    part.media_type(),
                    content_id,
                    part.bytes().to_vec(),
                ));
            }
        }
    }
    attachments
}

/// A display file name for a re-attached inline image, derived from its media type and
/// `Content-ID`. Cosmetic only: the part is addressed by its `Content-ID`, not its name.
fn inline_file_name(part: &InlinePart) -> String {
    let ext = match part.media_type().rsplit('/').next() {
        Some("jpeg") => "jpg",
        Some("svg+xml") => "svg",
        Some(sub) if !sub.is_empty() && sub.bytes().all(|b| b.is_ascii_alphanumeric()) => sub,
        _ => "img",
    };
    let base: String = part
        .content_id()
        .chars()
        .take_while(|&c| c != '@')
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = if base.is_empty() {
        "image"
    } else {
        base.as_str()
    };
    format!("{base}.{ext}")
}
