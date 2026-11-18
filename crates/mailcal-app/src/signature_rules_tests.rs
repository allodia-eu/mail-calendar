use mailcal_viewmodel::{SignatureImageRefusal, SignatureRefusal};

use super::{SIGNATURE_IMAGE_LIMIT_BYTES, body_images, signature_image, signature_name};

/// A 1×1 transparent PNG, base64.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

const TOO_LARGE: SignatureImageRefusal = SignatureImageRefusal::TooLarge {
    limit_bytes: SIGNATURE_IMAGE_LIMIT_BYTES,
};

#[test]
fn png_jpeg_and_gif_are_accepted_whatever_their_case() {
    for media_type in [
        "image/png",
        "IMAGE/JPEG",
        "image/Gif",
        "image/png; name=logo",
    ] {
        assert_eq!(signature_image(media_type, 10), Ok(()), "{media_type}");
    }
}

/// SVG is a document that can run scripts and fetch from outside, and HEIC and WebP are not shown
/// by every recipient's mail app.
#[test]
fn every_other_type_is_refused() {
    for media_type in [
        "image/svg+xml",
        "image/heic",
        "image/webp",
        "text/html",
        "",
        "image",
    ] {
        assert_eq!(
            signature_image(media_type, 10),
            Err(SignatureImageRefusal::Format),
            "{media_type}"
        );
    }
}

#[test]
fn a_picture_over_the_limit_is_refused_and_one_at_it_is_not() {
    assert_eq!(
        signature_image("image/png", SIGNATURE_IMAGE_LIMIT_BYTES),
        Ok(())
    );
    assert_eq!(
        signature_image("image/png", SIGNATURE_IMAGE_LIMIT_BYTES + 1),
        Err(TOO_LARGE)
    );
}

#[test]
fn a_name_is_trimmed_and_a_blank_one_refused() {
    assert_eq!(signature_name("  Work "), Ok("Work".to_owned()));
    assert_eq!(signature_name(" \t\n"), Err(SignatureRefusal::BlankName));
    assert_eq!(signature_name(""), Err(SignatureRefusal::BlankName));
}

#[test]
fn a_body_without_pictures_or_with_an_accepted_one_passes() {
    assert_eq!(body_images("<p>Alice</p>"), Ok(()));
    let body = format!(r#"<p>Alice</p><img src="data:image/png;base64,{PNG}" alt="">"#);
    assert_eq!(body_images(&body), Ok(()));
}

/// The save judges each picture by its bytes: a file named and typed as a PNG that holds an SVG
/// is refused, as is an SVG that says what it is.
#[test]
fn a_picture_whose_bytes_are_not_what_it_claims_is_refused() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let svg = STANDARD.encode(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#);
    for uri in [
        format!("data:image/png;base64,{svg}"),
        format!("data:image/svg+xml;base64,{svg}"),
        "data:image/png;base64,not base64".to_owned(),
    ] {
        let body = format!(r#"<img src="{uri}">"#);
        assert_eq!(
            body_images(&body),
            Err(SignatureImageRefusal::Format),
            "{uri}"
        );
    }
}

#[test]
fn a_body_with_one_picture_over_the_limit_is_refused() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut bytes = STANDARD.decode(PNG).unwrap();
    bytes.resize(usize::try_from(SIGNATURE_IMAGE_LIMIT_BYTES).unwrap() + 1, 0);
    let large = STANDARD.encode(&bytes);
    let body = format!(
        r#"<img src="data:image/png;base64,{PNG}"><img src="data:image/png;base64,{large}">"#
    );
    assert_eq!(body_images(&body), Err(TOO_LARGE));
}
