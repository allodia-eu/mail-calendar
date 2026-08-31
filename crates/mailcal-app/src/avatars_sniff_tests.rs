//! What the core will and will not hand a client as a photo.
//!
//! Pure checks over bytes and files, split from `avatars_tests.rs` for the 500-line limit;
//! and they read better apart: everything there needs a running app, nothing here does.

use super::{PhotoState, is_raster, usable};

/// A one-pixel PNG; real magic bytes, so the sniffing under test sees what it would in life.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01";

/// A fresh directory under the system temp dir, for the sample files.
fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-avatars-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// **The security assertion.** A provider's `media_type` is remote content describing itself,
/// so the bytes decide. SVG is the one that matters: it is script-capable, has no magic
/// number, and nothing in `rendering-security.md` permits it near a client surface: so it can
/// never pass, and no allow-list entry is needed to keep it out.
#[test]
fn only_the_raster_formats_every_platform_decodes_are_accepted() {
    assert!(is_raster(b"\x89PNG\r\n\x1a\n\x00\x00"));
    assert!(is_raster(b"\xff\xd8\xff\xe0\x00\x10"));
    assert!(is_raster(b"GIF87a\x01\x00"));
    assert!(is_raster(b"GIF89a\x01\x00"));
    assert!(is_raster(b"RIFF\x24\x00\x00\x00WEBPVP8 "));

    assert!(!is_raster(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"));
    assert!(!is_raster(b"<?xml version=\"1.0\"?><svg>"));
    assert!(!is_raster(b"<!DOCTYPE html><html>"));
    assert!(
        !is_raster(b"RIFF\x24\x00\x00\x00WAVEfmt "),
        "RIFF alone is not WebP"
    );
    assert!(!is_raster(b""));
    assert!(!is_raster(b"\x89PNG"), "a truncated signature is not a PNG");
}

/// A photo past the cap is refused rather than handed to a client that would decode it per
/// row, and a file that is not there at all is simply not a photo.
#[test]
fn an_oversized_or_missing_file_is_not_a_photo() {
    let dir = scratch_dir("usable");

    let good = dir.join("good.blob");
    std::fs::write(&good, PNG).unwrap();
    assert_eq!(
        usable(&good),
        PhotoState::File(good.display().to_string()),
        "a small PNG is exactly what this is for"
    );

    let huge = dir.join("huge.blob");
    let mut bytes = PNG.to_vec();
    bytes.resize(3 * 1024 * 1024, 0);
    std::fs::write(&huge, &bytes).unwrap();
    assert_eq!(
        usable(&huge),
        PhotoState::None,
        "past the cap, even as a PNG"
    );

    assert_eq!(usable(&dir.join("absent.blob")), PhotoState::None);
    let _ = std::fs::remove_dir_all(&dir);
}
