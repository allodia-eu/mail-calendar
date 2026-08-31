use super::sanitize;

#[test]
fn strips_script_tags_and_their_content() {
    let out = sanitize("<p>hi</p><script>steal(document.cookie)</script>").html;
    assert!(out.contains("hi"));
    assert!(!out.contains("<script"));
    assert!(!out.contains("steal"));
}

#[test]
fn strips_inline_event_handlers() {
    let out = sanitize(r#"<a href="https://ok.example" onclick="evil()">x</a>"#).html;
    assert!(!out.contains("onclick"));
    assert!(!out.contains("evil"));
    assert!(out.contains("https://ok.example"));
}

#[test]
fn preserves_inline_css_so_mail_is_legible() {
    // The reported bug: styling was stripped, so blue text rendered black. Inline
    // colours/fonts must survive (the WebView, not stripping, is the boundary).
    let out = sanitize(r#"<p style="color:#0000ff;font-weight:bold">blue</p>"#).html;
    assert!(out.contains("blue"));
    assert!(out.contains("style="));
    assert!(out.contains("#0000ff") || out.contains("color"));
}

#[test]
fn preserves_style_blocks_and_classes() {
    // Marketing mail puts media queries / class rules in <style>; child combinators and
    // @media must survive un-escaped, or the CSS breaks.
    let out = sanitize(
        "<style>@media (max-width:600px){.x{color:blue}} .a>.b{margin:0}</style>\
             <p class=\"x\">y</p>",
    )
    .html;
    assert!(out.contains("<style"));
    assert!(out.contains("@media"));
    assert!(out.contains(".a>.b") || out.contains(".a >.b") || out.contains(".a > .b"));
    assert!(out.contains("class=\"x\""));
}

#[test]
fn keeps_remote_images_but_flags_them() {
    // Remote images are kept (the host's CSP gates the actual load); the flag tells the
    // host to offer the "load remote images" prompt.
    let result = sanitize(r#"<img src="https://tracker.example/p.gif?id=42">"#);
    assert!(result.html.contains("tracker.example"));
    assert!(result.has_remote_images);
    // A CSS background image is flagged too.
    assert!(
        sanitize(r#"<div style="background:url(https://x.example/a.png)">z</div>"#)
            .has_remote_images
    );
}

#[test]
fn flags_remote_css_variants_a_flat_scan_misses() {
    // url() with whitespace after the paren, @import string form, protocol-relative src,
    // and a remote ref inside a <style> block; all must set has_remote_images.
    assert!(
        sanitize(r#"<div style="background:url( https://x.example/a.png )">z</div>"#)
            .has_remote_images
    );
    assert!(
        sanitize(
            "<style>@media all{body{background:url('https://x.example/a.png')}}</style><p>x</p>"
        )
        .has_remote_images
    );
    assert!(
        sanitize("<style>@import \"https://x.example/x.css\";</style><p>x</p>").has_remote_images
    );
    assert!(sanitize(r#"<img src="//x.example/a.png">"#).has_remote_images);
    // A plain remote link is still not a load.
    assert!(!sanitize(r#"<a href="https://x.example">l</a>"#).has_remote_images);
}

#[test]
fn inline_data_images_and_links_are_not_flagged_as_remote() {
    let result = sanitize(
        r#"<img src="data:image/png;base64,iVBORw0KGgo="><a href="https://x.example">l</a>"#,
    );
    assert!(result.html.contains("data:image/png;base64"));
    // A plain link is not a remote *load*, and a data: image is local: no prompt.
    assert!(!result.has_remote_images);
}

#[test]
fn preserves_presentational_table_attributes() {
    // Responsive mail relies on `border="0"` keeping border-width 0 so a mobile
    // `border-style: solid !important` rule stays invisible; dropping it paints a box.
    let out = sanitize(
            r#"<table border="0" cellspacing="0" cellpadding="0" width="600"><tr><td>x</td></tr></table>"#,
        )
        .html;
    assert!(out.contains(r#"border="0""#));
    assert!(out.contains(r#"cellspacing="0""#));
    assert!(out.contains(r#"cellpadding="0""#));
}

#[test]
fn drops_the_document_title_text() {
    // The title (often a subject copy) must not leak in as body text, but a <style>
    // sibling in <head> must still survive.
    let out = sanitize(
        "<head><title>Subject leak</title><style>.x{color:red}</style></head>\
             <body><p>visible</p></body>",
    )
    .html;
    assert!(!out.contains("Subject leak"));
    assert!(out.contains("visible"));
    assert!(out.contains(".x{color:red}"));
}

#[test]
fn strips_iframes_and_objects() {
    let out = sanitize(r#"<iframe src="https://x.example"></iframe><b>kept</b>"#).html;
    assert!(out.contains("kept"));
    assert!(!out.contains("iframe"));
}

#[test]
fn blocks_data_uri_links() {
    let out = sanitize(r#"<a href="data:text/html,<script>alert(1)</script>">x</a>"#).html;
    assert!(!out.contains("data:text/html"));
}

#[test]
fn strips_custom_scheme_links_so_they_match_the_launch_policy() {
    // A custom app-scheme href is dropped by the sanitiser (it isn't in the launchable
    // allowlist), so it can't even be clicked; keeping the sanitiser and the launch
    // policy (`should_open_external_link`) in agreement on the same scheme set.
    let out = sanitize(r#"<a href="myapp://home">open</a>"#).html;
    assert!(out.contains("open")); // link text survives
    assert!(!out.contains("myapp://")); // but the unlaunchable href is gone
}

use super::should_open_external_link;

#[test]
fn opens_only_safe_link_schemes() {
    assert!(should_open_external_link("https://example.com/path?q=1"));
    assert!(should_open_external_link("http://example.com"));
    assert!(should_open_external_link("mailto:a@b.example"));
    // The scheme is matched case-insensitively.
    assert!(should_open_external_link("HTTPS://Example.COM"));
}

#[test]
fn does_not_open_unsafe_relative_or_custom_schemes() {
    // Mail is hostile input: custom app deep-links, executable/document schemes, other
    // schemes outside the strict allowlist, and non-`scheme:` inputs never open. See
    // docs/rendering-security.md.
    for url in [
        "myapp://home",
        "javascript:alert(1)",
        "data:text/html,<script>",
        "file:///etc/passwd",
        "tel:+15551234567",
        "//example.com/proto-relative",
        "/relative/path",
        "notaurl",
        ":no-scheme",
        "",
    ] {
        assert!(!should_open_external_link(url), "should not open: {url:?}");
    }
}

#[test]
fn does_not_panic_on_non_ascii_link_href() {
    // Regression: is_data_uri must not byte-slice across a UTF-8 char boundary: a link
    // href with a multi-byte char around byte 5 used to panic the whole sanitise pass.
    assert!(
        sanitize(r#"<a href="abcdé fghij">x</a>"#)
            .html
            .contains('x')
    );
    // A char straddling byte 5 (the historical crash) and a short multi-byte value.
    let _ = sanitize("<a href=\"aaa😀bbb\">y</a>");
    let _ = sanitize("<a href=\"é\">z</a>");
}

#[test]
fn empty_or_textonly_input_is_safe() {
    assert_eq!(sanitize("").html, "");
    assert_eq!(sanitize("just text").html, "just text");
    assert!(!sanitize("just text").has_remote_images);
}

use super::render_document;

#[test]
fn render_document_embeds_the_body_and_a_strict_csp() {
    let doc = render_document("<p>hi</p>", false);
    assert!(doc.contains("<p>hi</p>"));
    // Scripts/frames/remote always blocked; inline styles allowed for presentation.
    assert!(doc.contains("default-src 'none'"));
    assert!(doc.contains("style-src 'unsafe-inline'"));
    // base-uri/form-action don't inherit default-src, so they're set explicitly.
    assert!(doc.contains("base-uri 'none'"));
    assert!(doc.contains("form-action 'none'"));
}

#[test]
fn render_document_forces_proportional_image_height() {
    // A wide image shrunk to fit the pane must keep its aspect ratio: `height:auto` is
    // `!important` so a message that pins an image's height (corporate signatures do)
    // can't leave the height fixed while the width shrinks, which squashes it.
    let doc = render_document("<img src=\"cid:x\">", false);
    assert!(doc.contains("max-width:100%"), "{doc}");
    assert!(doc.contains("height:auto!important"), "{doc}");
}

#[test]
fn render_document_gates_remote_images_on_the_flag() {
    // Blocked by default: only inline data: images.
    let blocked = render_document("<img src=\"https://x/a.png\">", false);
    assert!(blocked.contains("img-src data:"));
    assert!(!blocked.contains("img-src https:"));
    // Allowed once the user opts in: remote http(s) images load.
    let allowed = render_document("<img src=\"https://x/a.png\">", true);
    assert!(allowed.contains("https:"));
    assert!(allowed.contains("http:"));
}

use engine_api::InlinePart;

use super::{base64_encode_into, inline_cid_images};

fn part(cid: &str, media: &str, bytes: &[u8]) -> InlinePart {
    InlinePart::new(cid, media, bytes.to_vec())
}

#[test]
fn sanitize_flags_inline_cid_references() {
    // A `cid:` image source sets the flag so the reading path fetches inline parts;
    // a body with none (and a plain remote image) does not.
    assert!(sanitize(r#"<img src="cid:logo@x">"#).has_cid_references);
    assert!(!sanitize(r#"<img src="https://x/a.png">"#).has_cid_references);
    assert!(!sanitize("<p>just text</p>").has_cid_references);
}

#[test]
fn inline_cid_resolves_to_a_data_uri_with_base64_bytes() {
    // `hello` base64-encodes to `aGVsbG8=`; the resolved `<img>` loads inline.
    let out = inline_cid_images(
        r#"<img src="cid:logo@x">"#,
        &[part("logo@x", "image/png", b"hello")],
    );
    assert!(
        out.contains("src=\"data:image/png;base64,aGVsbG8=\""),
        "{out}"
    );
    assert!(!out.contains("cid:"), "the cid reference is gone: {out}");
}

#[test]
fn inline_cid_leaves_unresolved_and_nonimage_refs_untouched() {
    let parts = [
        part("present@x", "image/gif", b"hello"),
        // A non-image inline part must never become a `data:` document.
        part("doc@x", "text/html", b"<script>evil()</script>"),
    ];
    // An id with no matching part stays a (broken) cid reference, as before.
    let missing = inline_cid_images(r#"<img src="cid:absent@x">"#, &parts);
    assert!(missing.contains(r#"src="cid:absent@x""#), "{missing}");
    // A matching but non-image part is left as cid: no data:text/html is emitted.
    let nonimage = inline_cid_images(r#"<img src="cid:doc@x">"#, &parts);
    assert!(nonimage.contains(r#"src="cid:doc@x""#), "{nonimage}");
    assert!(!nonimage.contains("data:text/html"), "{nonimage}");
    assert!(!nonimage.contains("data:"), "{nonimage}");
}

#[test]
fn inline_cid_resolves_every_reference() {
    let html = r#"<img src="cid:a"><p>x</p><img src="cid:b">"#;
    let out = inline_cid_images(
        html,
        &[
            part("a", "image/gif", b"hello"),
            part("b", "image/jpeg", b"world"),
        ],
    );
    assert!(out.contains("data:image/gif;base64,aGVsbG8="), "{out}");
    assert!(out.contains("data:image/jpeg;base64,d29ybGQ="), "{out}");
    assert!(!out.contains("cid:"), "{out}");
    assert!(
        out.contains("<p>x</p>"),
        "surrounding markup preserved: {out}"
    );
}

#[test]
fn inline_cid_allows_svg_but_rejects_injection_in_media_type() {
    // SVG via <img> is rendered non-scripted and the CSP has no script-src, so it is
    // allowed.
    let svg = inline_cid_images(
        r#"<img src="cid:s">"#,
        &[part("s", "image/svg+xml", b"<svg/>")],
    );
    assert!(svg.contains("data:image/svg+xml;base64,"), "{svg}");
    // A media type smuggling extra `data:` parameters / an attribute break is rejected
    // (not image/* after the safe-char check), leaving the ref inert.
    let evil = inline_cid_images(
        r#"<img src="cid:s">"#,
        &[part("s", "image/png\";onerror=alert(1)//", b"x")],
    );
    assert!(evil.contains(r#"src="cid:s""#), "{evil}");
    assert!(!evil.contains("onerror"), "{evil}");
}

#[test]
fn inline_cid_is_a_noop_without_parts_or_references() {
    assert_eq!(
        inline_cid_images("<p>no images</p>", &[]),
        "<p>no images</p>"
    );
    let parts = [part("x", "image/png", b"hello")];
    // Parts present but the body references none of them: unchanged.
    assert_eq!(
        inline_cid_images("<p>no images</p>", &parts),
        "<p>no images</p>"
    );
}

#[test]
fn inline_cid_never_panics_on_hostile_input() {
    let parts = [part("é@x", "image/png", b"hello")];
    // Multi-byte chars around the cid value, and a `src="cid:` with no closing quote,
    // must not slice a char boundary or panic.
    let _ = inline_cid_images(r#"<img alt="café" src="cid:é@x">"#, &parts);
    let _ = inline_cid_images(r#"<img src="cid:unterminated"#, &parts);
    let _ = inline_cid_images("src=\"cid:😀\"", &parts);
}

use super::restore_cid_images;

#[test]
fn restore_cid_round_trips_inline_resolution_preserving_the_original_id() {
    // cid -> data (reading) -> cid (send) returns the exact original markup, and the part
    // is reported so the caller can re-attach it with its original Content-ID.
    let parts = [part("logo@allodia", "image/png", b"hello")];
    let resolved = inline_cid_images(r#"<img src="cid:logo@allodia">"#, &parts);
    assert!(
        resolved.contains("data:image/png;base64,aGVsbG8="),
        "{resolved}"
    );

    let part_refs: Vec<&InlinePart> = parts.iter().collect();
    let (restored, matched) = restore_cid_images(&resolved, &part_refs);
    assert_eq!(restored, r#"<img src="cid:logo@allodia">"#);
    assert!(!restored.contains("data:"), "{restored}");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].content_id(), "logo@allodia");
}

#[test]
fn restore_cid_leaves_foreign_data_images_and_is_a_noop_without_parts() {
    let parts = [part("a@x", "image/png", b"hello")];
    let part_refs: Vec<&InlinePart> = parts.iter().collect();
    // A data: image whose bytes match no part (base64 of "world") stays a data: URI.
    let foreign = r#"<img src="data:image/png;base64,d29ybGQ=">"#;
    let (restored, matched) = restore_cid_images(foreign, &part_refs);
    assert_eq!(restored, foreign);
    assert!(matched.is_empty());
    // No parts at all: unchanged.
    let (r2, m2) = restore_cid_images(foreign, &[]);
    assert_eq!(r2, foreign);
    assert!(m2.is_empty());
}

#[test]
fn restore_cid_anchors_on_the_full_src_so_a_prefix_part_cannot_corrupt_another() {
    // Part A's bytes ("hi", base64 "aGk="; wait: padded) vs a longer part B whose base64
    // starts with A's. Use lengths %3==0 so A's base64 has no padding and is a literal
    // prefix of B's: A = "abc" (YWJj), B = "abcdef" (YWJjZGVm). Anchoring the match on the
    // closing quote keeps A's URI from matching inside B's `src`.
    let parts = [
        part("a@x", "image/png", b"abc"),
        part("b@x", "image/png", b"abcdef"),
    ];
    let part_refs: Vec<&InlinePart> = parts.iter().collect();
    let html = concat!(
        r#"<img src="data:image/png;base64,YWJj">"#,
        r#"<img src="data:image/png;base64,YWJjZGVm">"#,
    );
    let (restored, matched) = restore_cid_images(html, &part_refs);
    // Each image becomes a cid: reference to its OWN part; B's src is not corrupted.
    assert_eq!(
        restored, r#"<img src="cid:a@x"><img src="cid:b@x">"#,
        "{restored}"
    );
    assert_eq!(matched.len(), 2);
}

#[test]
fn base64_matches_rfc4648_vectors() {
    let enc = |s: &[u8]| {
        let mut out = String::new();
        base64_encode_into(s, &mut out);
        out
    };
    assert_eq!(enc(b""), "");
    assert_eq!(enc(b"f"), "Zg==");
    assert_eq!(enc(b"fo"), "Zm8=");
    assert_eq!(enc(b"foo"), "Zm9v");
    assert_eq!(enc(b"foob"), "Zm9vYg==");
    assert_eq!(enc(b"fooba"), "Zm9vYmE=");
    assert_eq!(enc(b"foobar"), "Zm9vYmFy");
}
