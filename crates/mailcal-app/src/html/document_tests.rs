//! What [`render_document`] wraps a sanitised fragment in: the strict CSP that makes rendering the
//! message's own CSS safe, the base stylesheet, the viewport that decides which of the message's
//! `@media` rules apply, and the canvas the page is painted on.
//!
//! Layer 2 of `docs/rendering-security.md` plus the sizing rules of `docs/reading-zoom.md`. What
//! the sanitiser does to the fragment before any of this is next door, in `super::tests`.

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
fn the_document_carries_one_policy_rather_than_two() {
    // A `content` attribute holds a policy LIST, and a comma starts a second policy. Every
    // policy in the list is enforced, so the document gets the intersection, and a comma where
    // a semicolon belongs cuts `default-src 'none'` off from the `style-src` and `img-src`
    // that soften it. Both then fall back to `'none'` and the reading pane renders the message
    // with no CSS at all, its own and the base sheet's alike, and blocks even a `data:` image.
    //
    // The test above cannot see that: every directive is still present, just in the wrong
    // half. Assert on the separators instead.
    for load_remote_images in [false, true] {
        let doc = render_document("<p>hi</p>", load_remote_images);
        let policy = csp_of(&doc);
        assert!(
            !policy.contains(','),
            "the CSP is two policies, not one: {policy}"
        );
        // The pair that has to stay together: the restriction and the one directive that makes
        // the message's own presentation possible under it.
        assert!(policy.contains("default-src 'none'"), "{policy}");
        assert!(policy.contains("style-src 'unsafe-inline'"), "{policy}");
    }
}

/// The reading document's Content-Security-Policy, as the web view reads it.
fn csp_of(doc: &str) -> &str {
    let after = doc
        .split_once("http-equiv=\"Content-Security-Policy\" content=\"")
        .expect("the reading document carries a CSP")
        .1;
    after
        .split_once('"')
        .expect("the content attribute is quoted")
        .0
}

#[test]
fn the_viewport_pins_the_layout_width_and_leaves_the_scale_free() {
    // `width=device-width` is what makes a message's own `@media (max-width: …)` rules see the
    // pane the reader actually has. The scale is deliberately *not* pinned: an explicit
    // `initial-scale` fixes the page at 1:1, which is precisely how both touch engines decide
    // not to shrink an over-wide message to fit (docs/reading-zoom.md). A fixed-width newsletter
    // would then hang off the right edge of a phone with no way back.
    let doc = render_document("<p>hi</p>", false);
    let viewport = viewport_of(&doc);
    assert!(viewport.contains("width=device-width"), "{viewport}");
    assert!(!viewport.contains("initial-scale"), "{viewport}");
    // `user-scalable=no` / a pinned `maximum-scale` would take the reader's pinch away.
    assert!(!viewport.contains("user-scalable"), "{viewport}");
    assert!(!viewport.contains("maximum-scale"), "{viewport}");
}

/// The reading document's viewport declaration, as the web view reads it.
fn viewport_of(doc: &str) -> &str {
    let after = doc
        .split_once("name=\"viewport\" content=\"")
        .expect("the reading document declares a viewport")
        .1;
    after
        .split_once('"')
        .expect("the content attribute is quoted")
        .0
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
fn the_canvas_is_the_page_a_message_is_drawn_on() {
    // The client paints `MESSAGE_CANVAS` behind the body area for the whole of an open, and
    // the document paints its own page inside that. Two spellings of "white" would show as a
    // seam on every message, and, because the client's half is also what fills the gap before
    // the body lands, as a flicker on every open. So they are one constant, and this is what
    // stops a future restyle from moving only one of them.
    let doc = render_document("<p>hi</p>", false);
    assert!(
        doc.contains(&format!("background:{}", super::MESSAGE_CANVAS.background)),
        "{doc}"
    );
    assert!(
        doc.contains(&format!("color:{}", super::MESSAGE_CANVAS.foreground)),
        "{doc}"
    );
    // `#rrggbb`, the one form every client's hex parser already reads. `#fff` would render
    // identically here and reach three of the four clients as nothing at all.
    for channel in [
        super::MESSAGE_CANVAS.background,
        super::MESSAGE_CANVAS.foreground,
    ] {
        assert_eq!(channel.len(), 7, "{channel}");
        assert!(
            channel.starts_with('#') && channel[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "{channel}"
        );
    }
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
