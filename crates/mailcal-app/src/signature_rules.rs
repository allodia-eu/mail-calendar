//! What a signature may hold (`docs/signatures.md`, "What a signature may hold"): a name, and
//! pictures every recipient's mail app shows. Each rule is one function, which a client asks
//! before the person presses (the editor's Save, its image picker, a paste) and the save itself
//! acts on, so the two cannot disagree.

use mailcal_viewmodel::{SignatureImageRefusal, SignatureRefusal};

use crate::{composer_image::raster_media_type, mail_compose_signature::decode_data_image};

/// The largest picture a signature may hold, in bytes.
///
/// A signature rides in **every** message the account sends, and base64 adds a third on top
/// while it is stored. Generous for a logo, and small enough that nobody notices it on the wire.
pub const SIGNATURE_IMAGE_LIMIT_BYTES: u64 = 512 * 1024;

/// The picture types a signature may hold: the ones Outlook, Gmail and Apple Mail all show. An
/// SVG is a document that can carry scripts and outside references, and is not shown reliably.
const ACCEPTED: [&str; 3] = ["image/png", "image/jpeg", "image/gif"];

/// Whether a picture of `media_type` and `size` bytes may go into a signature. The media type is
/// matched without regard to case, and a parameter after it is ignored.
///
/// # Errors
///
/// [`SignatureImageRefusal::Format`] for any other type, and
/// [`SignatureImageRefusal::TooLarge`] for one over [`SIGNATURE_IMAGE_LIMIT_BYTES`].
pub fn signature_image(media_type: &str, size: u64) -> Result<(), SignatureImageRefusal> {
    let essence = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !ACCEPTED.contains(&essence.as_str()) {
        return Err(SignatureImageRefusal::Format);
    }
    if size > SIGNATURE_IMAGE_LIMIT_BYTES {
        return Err(SignatureImageRefusal::TooLarge {
            limit_bytes: SIGNATURE_IMAGE_LIMIT_BYTES,
        });
    }
    Ok(())
}

/// The name a signature is stored under: `name` without the spaces around it.
///
/// # Errors
///
/// [`SignatureRefusal::BlankName`] when nothing is left.
pub fn signature_name(name: &str) -> Result<String, SignatureRefusal> {
    let name = name.trim();
    if name.is_empty() {
        Err(SignatureRefusal::BlankName)
    } else {
        Ok(name.to_owned())
    }
}

/// Whether every picture in a sanitised signature body passes [`signature_image`], judged by its
/// bytes as well as the type it claims: a file can say it is a PNG and be something else.
pub(crate) fn body_images(html: &str) -> Result<(), SignatureImageRefusal> {
    // The sanitiser writes every attribute double-quoted, and a `data:` URI holds no quote.
    const SRC: &str = "src=\"data:";
    let mut rest = html;
    while let Some(start) = rest.find(SRC) {
        let value = &rest[start + SRC.len() - "data:".len()..];
        let end = value.find('"').unwrap_or(value.len());
        let uri = &value[..end];
        let (media_type, bytes) = decode_data_image(uri).ok_or(SignatureImageRefusal::Format)?;
        signature_image(&media_type, bytes.len() as u64)?;
        let sniffed = raster_media_type(&bytes).ok_or(SignatureImageRefusal::Format)?;
        if !ACCEPTED.contains(&sniffed) {
            return Err(SignatureImageRefusal::Format);
        }
        rest = &value[end..];
    }
    Ok(())
}

#[cfg(test)]
#[path = "signature_rules_tests.rs"]
mod tests;
