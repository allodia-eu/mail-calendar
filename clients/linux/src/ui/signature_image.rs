//! A picture going into a signature: the core's rule asked of it, the `data:` URI it becomes, and
//! the words for every answer.
//!
//! A signature stores its images inline as `data:` URIs: right for the library (one
//! self-contained file) and what the core rewrites to a `cid:` part on send
//! ([`docs/signatures.md`](../../../../docs/signatures.md)). Which pictures a signature may hold
//! is the core's one rule (`signature_image_refusal`); this half is pure so it is testable without
//! a file picker, and the GTK half reads the bytes and shows the message.

use gtk::glib;
use mailcal_bindings::{SignatureImageRefusal, SignatureRefusal};

use crate::l10n;

/// What offering a picture to a signature yielded.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SignatureImage {
    /// A `data:image/…;base64,…` URI ready to insert at the caret, and the alt text to give it.
    DataUrl { value: String, alt_text: String },
    /// The core's rule refused it.
    Refused(SignatureImageRefusal),
    /// It could not be read.
    Failed,
}

/// Why the core would refuse a picture of `media_type` and `size` bytes, asked before a picked
/// file is read. A picture whose type nobody reported is asked about with no type, which the core
/// refuses as not a picture it takes.
pub(super) fn refusal(media_type: Option<&str>, size: u64) -> Option<SignatureImageRefusal> {
    mailcal_bindings::signature_image_refusal(media_type.unwrap_or_default().to_owned(), size)
}

/// `bytes` of `media_type` as a picture to insert, once the core's rule takes them.
pub(super) fn signature_image(
    bytes: &[u8],
    media_type: Option<&str>,
    alt_text: &str,
) -> SignatureImage {
    if let Some(refused) = refusal(media_type, bytes.len() as u64) {
        return SignatureImage::Refused(refused);
    }
    let Some(media_type) = media_type.filter(|_| !bytes.is_empty()) else {
        return SignatureImage::Failed;
    };
    SignatureImage::DataUrl {
        value: format!("data:{media_type};base64,{}", glib::base64_encode(bytes)),
        alt_text: alt_text.to_owned(),
    }
}

/// What the person is told about a picture the core refused. The limit is the core's, named in
/// the words this client gives any file size.
pub(super) fn image_refusal_text(refusal: SignatureImageRefusal) -> String {
    match refusal {
        SignatureImageRefusal::Format => l10n::settings_signatures_image_format().to_owned(),
        SignatureImageRefusal::TooLarge { limit_bytes } => {
            l10n::settings_signatures_image_too_large(&glib::format_size(limit_bytes))
        }
    }
}

/// What the person is told about a save the core refused.
pub(super) fn refusal_text(refusal: SignatureRefusal) -> String {
    match refusal {
        SignatureRefusal::BlankName => l10n::settings_signatures_name_required().to_owned(),
        SignatureRefusal::Gone => l10n::settings_signatures_gone().to_owned(),
        SignatureRefusal::Image { refusal } => image_refusal_text(refusal),
    }
}

#[cfg(test)]
mod tests {
    use gtk::glib;
    use mailcal_bindings::{SignatureImageRefusal, SignatureRefusal};

    use super::{SignatureImage, image_refusal_text, refusal_text, signature_image};
    use crate::l10n;

    /// A pasted picture is offered through this function, so it meets the core's rule for a
    /// signature: a type the message body would take and a signature does not is refused, and so
    /// is a picture over the signature's limit.
    #[test]
    fn a_pasted_picture_is_judged_by_the_cores_rule() {
        assert_eq!(
            signature_image(b"RIFF\0\0\0\0WEBP", Some("image/webp"), ""),
            SignatureImage::Refused(SignatureImageRefusal::Format)
        );
        assert_eq!(
            signature_image(b"%PDF-1.7", Some("application/pdf"), ""),
            SignatureImage::Refused(SignatureImageRefusal::Format)
        );
        assert_eq!(
            signature_image(b"\x89PNG", None, ""),
            SignatureImage::Refused(SignatureImageRefusal::Format)
        );
        let Some(SignatureImageRefusal::TooLarge { limit_bytes }) =
            super::refusal(Some("image/png"), u64::MAX)
        else {
            panic!("the core caps a signature's picture");
        };
        let over = usize::try_from(limit_bytes + 1).expect("the limit fits a usize");
        assert_eq!(
            signature_image(&vec![0_u8; over], Some("image/png"), ""),
            SignatureImage::Refused(SignatureImageRefusal::TooLarge { limit_bytes })
        );
    }

    #[test]
    fn a_picture_the_core_takes_becomes_a_self_contained_data_uri() {
        assert_eq!(
            signature_image(&[0x01, 0x02, 0x03], Some("image/png"), "logo"),
            SignatureImage::DataUrl {
                value: "data:image/png;base64,AQID".to_owned(),
                alt_text: "logo".to_owned(),
            }
        );
        assert_eq!(
            signature_image(&[], Some("image/png"), ""),
            SignatureImage::Failed
        );
    }

    #[test]
    fn each_refusal_is_worded_from_the_catalog_with_the_cores_limit() {
        assert_eq!(
            image_refusal_text(SignatureImageRefusal::Format),
            l10n::settings_signatures_image_format()
        );
        // The limit named is the one the core sent, not one this client knows.
        for limit_bytes in [512 * 1024, 2_000_000] {
            assert_eq!(
                image_refusal_text(SignatureImageRefusal::TooLarge { limit_bytes }),
                l10n::settings_signatures_image_too_large(&glib::format_size(limit_bytes))
            );
        }
        assert_eq!(
            refusal_text(SignatureRefusal::BlankName),
            l10n::settings_signatures_name_required()
        );
        assert_eq!(
            refusal_text(SignatureRefusal::Gone),
            l10n::settings_signatures_gone()
        );
        assert_eq!(
            refusal_text(SignatureRefusal::Image {
                refusal: SignatureImageRefusal::Format
            }),
            l10n::settings_signatures_image_format()
        );
    }
}
