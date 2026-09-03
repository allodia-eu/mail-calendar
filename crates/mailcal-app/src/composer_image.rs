//! Reading a picture off disk for the composer body.
//!
//! A file dropped on the composer can go two ways: attached, which stays a host-staged path Rust
//! streams from disk on send, or **shown in the message**, which needs the bytes inside the editor
//! document as a `data:` URI (`mailcal_composer::DraftAttachment::data_url`). This is the second,
//! and it lives in the core so all four clients inherit one answer to the three questions a host
//! would otherwise each answer differently: how large is too large, what counts as a picture, and
//! what the URI looks like.
//!
//! **The file's name is not the check.** A host hands over whatever the user dropped, and an
//! extension (or a desktop environment's guess at a media type) describes the file rather than
//! proves it. So the bytes are sniffed, exactly as an avatar's are, and only the raster formats
//! every platform decodes natively pass. SVG can never pass: it has no magic number and is
//! script-capable.

use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// The largest picture that may be shown in the message body.
///
/// Base64 inflates by a third and the whole document crosses the FFI as one string, so the cost of
/// a picture in the body is several times its size on disk. Well above a screenshot or a phone
/// photo, well below what a mail server accepts; a larger file is still attachable, which is what
/// the other answer to the drop question is for.
const MAX_INLINE_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// The `data:image/…;base64,…` URI for the picture at `path`, or `None` when it is not a raster
/// image, is larger than the cap above, or cannot be read.
///
/// Reads the whole file, so a host calls it off whatever thread draws its window.
pub fn image_data_url(path: &Path) -> Option<String> {
    let length = std::fs::metadata(path).ok()?.len();
    if length > MAX_INLINE_IMAGE_BYTES {
        log::info!("composer: picture of {length} bytes refused as oversized");
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let Some(media_type) = raster_media_type(&bytes) else {
        log::info!("composer: dropped file refused: not a raster image");
        return None;
    };
    Some(format!(
        "data:{media_type};base64,{}",
        STANDARD.encode(&bytes)
    ))
}

/// The media type `head` identifies by magic number, or `None` for anything else.
///
/// The list is the one every client decodes natively, and is deliberately closed: a format nothing
/// can render is a broken picture in the recipient's mailbox, and a script-capable one has no
/// business behind an `<img>`.
pub(crate) fn raster_media_type(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{image_data_url, raster_media_type};

    /// A 1×1 transparent PNG.
    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R',
    ];

    #[test]
    fn each_raster_format_is_identified_by_its_magic_number() {
        assert_eq!(raster_media_type(PNG), Some("image/png"));
        assert_eq!(raster_media_type(b"\xff\xd8\xff\xe0"), Some("image/jpeg"));
        assert_eq!(raster_media_type(b"GIF89a...."), Some("image/gif"));
        assert_eq!(raster_media_type(b"RIFF____WEBPVP8 "), Some("image/webp"));
    }

    #[test]
    fn a_script_capable_or_unknown_file_is_not_a_picture() {
        // The two that matter: an SVG named `.png` (script-capable, no magic number) and a PDF
        // the user meant to attach rather than show. Neither may reach an `<img>`.
        assert_eq!(
            raster_media_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
            None
        );
        assert_eq!(raster_media_type(b"%PDF-1.7"), None);
        assert_eq!(raster_media_type(b""), None);
        assert_eq!(raster_media_type(b"RIFF____AVI "), None);
    }

    #[test]
    fn a_picture_becomes_a_base64_data_uri_and_a_non_picture_becomes_nothing() {
        let dir = std::env::temp_dir().join("mailcal-composer-image-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let picture = dir.join("shot.png");
        std::fs::write(&picture, PNG).expect("write picture");
        // The extension says PNG and the bytes say otherwise; the bytes decide.
        let liar = dir.join("logo.png");
        std::fs::write(&liar, b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>").expect("write");

        let url = image_data_url(&picture).expect("a raster file yields a data URI");
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
        assert!(image_data_url(&liar).is_none());
        assert!(image_data_url(&dir.join("absent.png")).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
