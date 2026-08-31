//! The size cap and media-type rule an image embedded in a signature must pass.
//!
//! A signature stores its images inline as `data:` URIs: right for the library (one
//! self-contained file) and what the core rewrites to a `cid:` part on send
//! ([`docs/signatures.md`](../../../../docs/signatures.md)). The pure half lives here so both
//! rules are testable without a file picker; the GTK half reads the bytes and shows the message.

use gtk::glib;

/// The cap on one embedded signature image, in bytes.
///
/// A signature rides in **every** message the account sends, so a 5 MB logo is 5 MB per mail,
/// and base64 adds a third on top. 512 KB is generous for a logo and small enough that nobody
/// notices it on the wire. Enforced where the file is picked, so the user is told; the core does
/// not police it.
pub(super) const LIMIT_BYTES: u64 = 512 * 1024;

/// What picking a file for a signature yielded.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SignatureImage {
    /// A `data:image/…;base64,…` URI ready to insert at the caret, and the alt text to give it.
    DataUrl { value: String, alt_text: String },
    /// The file is over [`LIMIT_BYTES`].
    TooLarge,
    /// The file could not be read, or is not an image.
    Failed,
}

/// The outcome for `bytes` of `media_type`.
///
/// The size check is separate from the read failure so the user is told *which* problem it is.
/// Anything that is not an `image/*` is refused here rather than embedded: the editor drops it
/// anyway (it accepts only `data:image/`), and the picker is the last place the user can be told.
pub(super) fn signature_image(
    bytes: &[u8],
    media_type: Option<&str>,
    alt_text: &str,
) -> SignatureImage {
    if bytes.len() as u64 > LIMIT_BYTES {
        return SignatureImage::TooLarge;
    }
    let Some(media_type) =
        media_type.filter(|kind| kind.to_ascii_lowercase().starts_with("image/"))
    else {
        return SignatureImage::Failed;
    };
    if bytes.is_empty() {
        return SignatureImage::Failed;
    }
    SignatureImage::DataUrl {
        value: format!(
            "data:{};base64,{}",
            media_type.to_ascii_lowercase(),
            glib::base64_encode(bytes)
        ),
        alt_text: alt_text.to_owned(),
    }
}

/// The cap as the message names it, in the same words this client already gives a file size
/// (the diagnostics log's size row) rather than a second convention.
pub(super) fn format_limit() -> String {
    glib::format_size(LIMIT_BYTES).into()
}

#[cfg(test)]
mod tests {
    use super::{LIMIT_BYTES, SignatureImage, format_limit, signature_image};

    #[test]
    fn an_image_within_the_cap_becomes_a_self_contained_data_uri() {
        let outcome = signature_image(&[0x01, 0x02, 0x03], Some("image/png"), "logo");

        assert_eq!(
            outcome,
            SignatureImage::DataUrl {
                value: "data:image/png;base64,AQID".to_owned(),
                alt_text: "logo".to_owned(),
            }
        );
    }

    #[test]
    fn the_cap_and_the_media_type_refusal_are_told_apart() {
        let oversized = vec![0u8; usize::try_from(LIMIT_BYTES).expect("cap fits a usize") + 1];

        // Two different messages for two different problems: "too large" names a cap the user can
        // meet, "couldn't be read" does not.
        assert_eq!(
            signature_image(&oversized, Some("image/png"), "logo"),
            SignatureImage::TooLarge
        );
        assert_eq!(
            signature_image(&[0x01], Some("application/pdf"), "doc"),
            SignatureImage::Failed
        );
        assert_eq!(
            signature_image(&[0x01], None, "unknown"),
            SignatureImage::Failed
        );
        assert_eq!(
            signature_image(&[], Some("image/png"), ""),
            SignatureImage::Failed
        );
    }

    #[test]
    fn a_media_type_the_server_shouted_still_matches() {
        // `content_type_get_mime_type` answers from the shared MIME database, which is lower case
        //; but the check must not be the thing that decides whether a logo embeds.
        assert!(matches!(
            signature_image(&[0x01], Some("IMAGE/PNG"), "logo"),
            SignatureImage::DataUrl { ref value, .. } if value.starts_with("data:image/png;base64,")
        ));
    }

    #[test]
    fn the_cap_is_named_in_the_message() {
        let limit = format_limit();

        assert!(!limit.is_empty());
        assert!(
            limit.contains(char::is_numeric),
            "the message has to name a size the user can compare against: {limit}"
        );
    }
}
