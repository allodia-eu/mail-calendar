//! The document a sanitised fragment is wrapped in before a host renders it: the strict CSP that
//! makes the message's own CSS safe to run, the page it is drawn on, the base stylesheet, and the
//! per-message sheet that reflows a message written for a wider pane ([`super::reflow`]).
//!
//! Layer 2 of `docs/rendering-security.md`, and rules 1 to 3 of `docs/reading-zoom.md`. What the
//! sanitiser does to the fragment before any of this is next door, in [`super`].

use std::sync::OnceLock;

use super::reflow;

/// The canvas a message is drawn on: the page's background, and the ink that stays legible
/// on it. `#rrggbb`, the form every client already parses for a calendar chip and an avatar.
///
/// It is the same in both themes because HTML mail is designed for a white page and the base
/// stylesheet pins one with `color-scheme: light`. That makes it **app chrome the client
/// also has to draw**: the body area is this canvas from the moment a message is opened,
/// while the body is still resolving, for a plain-text body, for a load error, so an open
/// that resolves in tens of milliseconds changes the text on the page and never repaints the
/// page itself. A client that left the gap transparent instead punched a hole in the canvas:
/// on a dark theme the body area went white, black, white, which reads as a flicker rather
/// than as a message opening (`docs/sync-progress.md`).
///
/// Shared rather than restated per client for the same reason the CSP is: the client's half
/// and the document's half cannot drift into two slightly different whites. The base stylesheet
/// interpolates this constant, so there is only ever one.
pub const MESSAGE_CANVAS: Canvas = Canvas {
    background: "#ffffff",
    foreground: "#1a1a1a",
};

/// A background and the ink that stays legible on it, both `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canvas {
    /// The page's fill.
    pub background: &'static str,
    /// The text colour on that fill.
    pub foreground: &'static str,
}

/// How far the document insets itself from the web view's edges, on every side.
///
/// Shared with [`reflow`], whose breakpoint is the width at which a message stops fitting: what it
/// has to fit inside is the pane **less this, twice**. A second copy of the number would put that
/// breakpoint 28px out, leaving a band of pane widths where a message is clipped and nothing
/// reflows it.
pub(super) const BODY_PADDING: u32 = 14;

/// The base stylesheet for the reading document: a readable default that the message's own
/// CSS overrides. `color-scheme: light` keeps the canvas white (HTML mail is designed for a
/// white background) rather than letting the WebView auto-darken it.
///
/// `img{max-width:100%;height:auto!important}` is the **one** rule the message may not
/// override: `max-width:100%` shrinks a wide image to fit a narrow reading pane, and the
/// `!important` `height:auto` forces the height to follow that shrunk width so the aspect
/// ratio is preserved. Without `!important`, an image whose height is pinned by the
/// message's own CSS or inline style (corporate signatures routinely do; `<img>` with a
/// fixed `height`) keeps that height while only its width shrinks, squashing it
/// horizontally. Auto height is always proportional scaling, so this never distorts an
/// image; it only overrides an explicit height that the width constraint would otherwise
/// fight.
///
/// Built once rather than per render: it interpolates a constant, so every message would
/// otherwise pay for the same string.
fn base_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let Canvas {
            background,
            foreground,
        } = MESSAGE_CANVAS;
        format!(
            ":root{{color-scheme:light}}\
             body{{margin:0;padding:{BODY_PADDING}px;background:{background};color:{foreground};\
             font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
             font-size:15px;line-height:1.5;overflow-wrap:break-word;\
             -webkit-text-size-adjust:100%}}\
             img{{max-width:100%;height:auto!important}}a{{color:#2864d6}}"
        )
    })
}

/// Wraps a sanitised body fragment in a complete HTML document; with a strict
/// Content-Security-Policy and base styling; ready for a host WebView to load. Shared in
/// Rust so the security boundary (the CSP) and the base presentation are **identical across
/// every client** (macOS/Android/Windows); a host only adds the unavoidably-native bits
/// (disable JavaScript, block navigation).
///
/// The CSP is the boundary that makes rendering the message's CSS safe: `default-src 'none'`
/// blocks scripts, frames, and every remote load; `style-src 'unsafe-inline'` allows the
/// message's presentational CSS. When `load_remote_images` is `false` (the default) `img-src
/// data:` blocks every remote image: so a message cannot phone home or track the open until
/// the user explicitly loads images (`north-star.md`), when `true`, remote http(s) images
/// (and CSS backgrounds) load. The host passes the user's per-message choice here and
/// re-renders.
///
/// The stylesheet is in two parts: the base sheet, the same for every message, and the reflow
/// sheet, built from **this** message's own widths, which is what makes a message written for
/// 600px readable in a narrower pane (`docs/reading-zoom.md`, rule 3).
///
/// The viewport declares a width and **no scale** (`docs/reading-zoom.md`).
/// `width=device-width` is what puts the message's own `@media (max-width: …)` rules in front of
/// the pane the reader actually has. Naming an `initial-scale` as well would pin the page at 1:1,
/// which is the condition under which Blink declines to shrink an over-wide message to fit, so a
/// fixed-width newsletter would hang off the right edge of a phone. Leaving the scale free is also
/// what leaves the reader their pinch, so no `user-scalable` or `maximum-scale` belongs here
/// either. A message cannot override any of this: the sanitiser drops `<meta>`.
#[must_use]
pub fn render_document(body_fragment: &str, load_remote_images: bool) -> String {
    let img_src = if load_remote_images {
        "https: http: data:"
    } else {
        "data:"
    };
    let base_css = base_css();
    // Built per message rather than once, because its breakpoint is this message's own width
    // (`reflow`). A message with nothing fixed in it produces no breakpoint and pays for a scan.
    let reflow_css = reflow::reflow_css(reflow::natural_width(body_fragment));
    // `base-uri`/`form-action` do NOT fall back to `default-src`, so set them explicitly:
    // even if a `<base>` or `<form>` ever survived sanitisation, it can't rebase relative
    // URLs or POST data off-host.
    //
    // ⚠️ Every separator below is a SEMICOLON. A `content` attribute is a policy *list*, and a
    // comma starts a second policy that is enforced alongside the first, so the document gets
    // the intersection. One comma here splits `default-src 'none'` away from the `style-src`
    // and `img-src` that soften it, both fall back to `'none'`, and the message renders with no
    // CSS at all and not even a `data:` image.
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
         base-uri 'none'; form-action 'none'; \
         img-src {img_src}; style-src 'unsafe-inline'; font-src data:\">\
         <meta name=\"viewport\" content=\"width=device-width\">\
         <style>{base_css}{reflow_css}</style></head><body>{body_fragment}</body></html>"
    )
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod document_tests;
