//! HTML sanitisation for the reading view.
//!
//! The engine returns a message's `text/html` part **unsanitized** (`mime.md`): mail HTML
//! is hostile input. Before any host renders it, we strip it to a safe, inert subset here;
//! one hardened sanitiser shared by every client, not re-implemented per platform: so a
//! malicious message cannot run script, escape its frame, or navigate the host away.
//!
//! What is removed ([`ammonia`]'s allowlist): scripts and their content, event handlers
//! (`on*`), `<iframe>`/`<object>`/`<embed>`/`<form>`, `<base>`/`<meta>` (so a message can't
//! set its own refresh/CSP), and any URL scheme other than http(s)/mailto/data/cid.
//!
//! What is **kept**: presentational HTML and CSS; inline `style`, `<style>` blocks, and
//! `class`; because stripping CSS makes real mail illegible (no colours, fonts, or
//! layout), and remote `<img>` sources. CSS and remote loads are made safe not by stripping
//! but by the host **WebView**, which is the security boundary: JavaScript is off, a strict
//! Content-Security-Policy blocks every remote load **by default** (so remote images don't
//! phone home / track opens until the user explicitly loads them; `north-star.md`), and all
//! navigation is blocked. [`sanitize`] reports [`Sanitized::has_remote_images`] so a host
//! can offer that "load remote images" confirmation only when there is something to load.

use std::{collections::HashSet, sync::OnceLock};

use ammonia::Builder;
use engine_api::InlinePart;

/// The sanitised body, whether it references remote resources (remote `<img>` or a CSS
/// `url(...)`), so a host can gate loading behind a "load remote images" confirmation, and
/// whether it references inline `cid:` resources that may need resolving.
pub(crate) struct Sanitized {
    /// The inert, safe-to-render HTML (a body fragment).
    pub html: String,
    /// Whether the body would load a remote resource if the host's CSP allowed it; the
    /// signal to offer the "load remote images" prompt.
    pub has_remote_images: bool,
    /// Whether the body references an inline `cid:` resource: the signal for the reading
    /// path to fetch the message's inline parts and resolve them ([`inline_cid_images`]),
    /// so an open with no inline images never pays for the parts fetch.
    pub has_cid_references: bool,
}

/// URL schemes a clicked link in a rendered message may be handed to the OS to open
/// ([`should_open_external_link`]). Mail is hostile input, so this is a deliberately strict
/// allowlist: only well-understood, low-risk schemes; never custom app schemes, `data:`,
/// `file:`, `javascript:`, etc. It is the **single source of truth** for the launch policy,
/// shared by every client through the FFI so they cannot drift, and it is also the set of
/// schemes the sanitiser keeps on `<a href>` (a link the sanitiser strips can never be
/// clicked, so the two must agree).
const EXTERNAL_LINK_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Extra schemes the sanitiser keeps for **inline resources** referenced by `src` (not
/// links): `data:` for inline images and `cid:` for referenced message parts. These never
/// open externally: only [`EXTERNAL_LINK_SCHEMES`] do.
const INLINE_RESOURCE_SCHEMES: [&str; 2] = ["data", "cid"];

/// Sanitises a message's raw `text/html` into an inert subset safe to render in a
/// locked-down WebView, preserving presentational CSS.
pub(crate) fn sanitize(html: &str) -> Sanitized {
    let cleaned = sanitizer().clean(html).to_string();
    let has_remote_images = references_remote_resource(&cleaned);
    let has_cid_references = references_cid(&cleaned);
    Sanitized {
        html: cleaned,
        has_remote_images,
        has_cid_references,
    }
}

/// The shared, immutable [`Builder`]: its allowlists never change, so it is configured once
/// and reused for every message open (ammonia's `clean` takes `&self`), rather than
/// rebuilt per call.
fn sanitizer() -> &'static Builder<'static> {
    static SANITIZER: OnceLock<Builder<'static>> = OnceLock::new();
    SANITIZER.get_or_init(|| {
        // Only safe URL schemes survive, notably no `javascript:`/`vbscript:`. Links use the
        // launchable allowlist (http(s)/mailto); `data:`/`cid:` are kept for inline `src`
        // resources (and `data:` *links* are blocked below: an executable document). Remote
        // http(s) image sources are kept but gated by the host's CSP. The link half of this
        // set is shared with the host's launch policy (see `EXTERNAL_LINK_SCHEMES`).
        let schemes: HashSet<&str> = EXTERNAL_LINK_SCHEMES
            .into_iter()
            .chain(INLINE_RESOURCE_SCHEMES)
            .collect();
        let mut builder = Builder::new();
        builder
            .url_schemes(schemes)
            // Keep presentational CSS (inline + blocks) and the attributes mail uses for
            // layout. The WebView (not stripping) contains it (scripting off, remote loads
            // blocked).
            .add_tags(["style"])
            .rm_clean_content_tags(["style"])
            // Drop `<title>`'s text too, not just the tag; otherwise the document title
            // (often a copy of the subject) leaks in as a stray line of body text.
            .add_clean_content_tags(["title"])
            // Presentational attributes mail uses for table layout. `border`/`cellspacing`/
            // `cellpadding` matter for correctness, not just looks: responsive mail commonly
            // sets `border="0"` and then `border-style: solid !important` in a mobile @media
            // rule, relying on the attribute's `border-width: 0` to keep it invisible; drop
            // the attribute and that rule paints a spurious box. (No URL or script.)
            .add_generic_attributes([
                "style",
                "class",
                "align",
                "valign",
                "bgcolor",
                "border",
                "cellspacing",
                "cellpadding",
                "width",
                "height",
            ])
            .attribute_filter(|element, attribute, value| match (element, attribute) {
                // A `data:` link can carry an executable document (e.g. data:text/html);
                // links may only point at real http(s)/mailto targets.
                ("a", "href") if is_data_uri(value) => None,
                _ => Some(value.into()),
            });
        builder
    })
}

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
             body{{margin:0;padding:14px;background:{background};color:{foreground};\
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
#[must_use]
pub fn render_document(body_fragment: &str, load_remote_images: bool) -> String {
    let img_src = if load_remote_images {
        "https: http: data:"
    } else {
        "data:"
    };
    let base_css = base_css();
    // `base-uri`/`form-action` do NOT fall back to `default-src`, so set them explicitly:
    // even if a `<base>` or `<form>` ever survived sanitisation, it can't rebase relative
    // URLs or POST data off-host.
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
         base-uri 'none', form-action 'none'; \
         img-src {img_src}; style-src 'unsafe-inline'; font-src data:\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <style>{base_css}</style></head><body>{body_fragment}</body></html>"
    )
}

/// Whether a link the user clicked in a rendered message should be handed to the OS default
/// browser/handler. The host calls this for a **user-activated** link tap and, on `true`,
/// opens the URL with the platform's launcher (`NSWorkspace` / `Intent.ACTION_VIEW` /
/// `Launcher.LaunchUriAsync`); on `false` it ignores the tap. The policy; the
/// `EXTERNAL_LINK_SCHEMES` allowlist; lives here so every client decides identically and
/// consistently with what the sanitiser keeps (mail is hostile input; see
/// `docs/rendering-security.md`).
#[must_use]
pub fn should_open_external_link(url: &str) -> bool {
    scheme_of(url).is_some_and(|scheme| EXTERNAL_LINK_SCHEMES.contains(&scheme.as_str()))
}

/// The lowercased URL scheme (the part before the first `:`), if `url` starts with a
/// syntactically valid RFC 3986 scheme; `None` for a relative URL or anything not in
/// `scheme:` form (those never open). Avoids a URL-parser dependency for a check this small,
/// and is byte-safe on hostile input (`split_once`/char iteration never slice mid-codepoint).
fn scheme_of(url: &str) -> Option<String> {
    let (scheme, _) = url.trim().split_once(':')?;
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// Whether `value` is an inline `data:` URI (scheme-only check, leading space tolerant).
///
/// Uses `get(..5)` rather than slicing `[..5]`: the value is hostile input, and a byte
/// slice would panic when byte 5 falls inside a multi-byte char (e.g. a link `href` with a
/// non-ASCII character), which would unwind through the whole sanitise pass.
fn is_data_uri(value: &str) -> bool {
    value
        .trim_start()
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

/// Whether the sanitised HTML references a remote resource that *loads*: a remote `<img>`
/// source, or a CSS `url(...)`/`@import` (inline **or** in a `<style>` block). Plain links
/// (`href`) are excluded: they don't auto-fetch, so they aren't a tracking vector. Scanned
/// on the serialized output so `<style>` blocks are covered (an attribute filter can't see
/// their text); ammonia normalises attribute quoting to `"`, so the `src="…"` checks hold.
fn references_remote_resource(html: &str) -> bool {
    let h = html.to_ascii_lowercase();
    h.contains("src=\"http")
        || h.contains("src=\"//")
        || css_marker_loads_remote(&h, "url(")
        || css_marker_loads_remote(&h, "@import")
}

/// Whether any `marker` in `h` is followed (after optional CSS whitespace and quotes) by a
/// remote URL (`http…` or a protocol-relative `//…`). Catches the variants a flat substring
/// match misses: `url( http`, `url('http')`, `@import "https://…"`. `h` must be lowercased.
fn css_marker_loads_remote(h: &str, marker: &str) -> bool {
    let mut rest = h;
    while let Some(i) = rest.find(marker) {
        let after = rest[i + marker.len()..].trim_start_matches([' ', '\t', '\n', '\r', '\'', '"']);
        if after.starts_with("http") || after.starts_with("//") {
            return true;
        }
        rest = &rest[i + marker.len()..];
    }
    false
}

/// Whether the sanitised HTML references an inline `cid:` image source; `src="cid:…"`, the
/// only place the sanitiser keeps a `cid:` scheme (an `<img>`'s `src`). The cheap signal the
/// reading path uses to decide whether to fetch the message's inline parts at all, so a
/// message with no inline images never pays for that fetch. Matched case-insensitively
/// because a host may author `CID:`/`SRC=`.
fn references_cid(html: &str) -> bool {
    html.to_ascii_lowercase().contains(CID_SRC_MARKER)
}

/// The `src="cid:` token (lowercased) that fronts every inline-image reference in ammonia's
/// serialized output (attributes are normalised to double quotes). `cid:` is kept by the
/// sanitiser only on an `<img src>`, so anchoring on `src="` confines the rewrite to image
/// sources and never touches a surviving `cid:` elsewhere.
const CID_SRC_MARKER: &str = "src=\"cid:";

/// Rewrites each resolvable inline `cid:` image reference in `html` to a self-contained
/// `data:` URI built from `parts`, so the existing reading-document CSP (`img-src data:`)
/// renders it without a network load and without the "load remote images" opt-in; inline
/// images are part of the message, not remote content.
///
/// Only an `<img src="cid:ID">` is rewritten (the sole place the sanitiser keeps `cid:`),
/// and only when `ID` resolves to an inline part whose media type is a safe `image/*`
/// (defensive: the rewrite must never emit a non-image `data:` document such as
/// `data:text/html`). An unresolved id, or a non-image part, is left untouched: an inert
/// broken image, exactly as before this resolution existed. Operates on byte indices found
/// at ASCII boundaries, so it never slices a multi-byte char in hostile input.
pub(crate) fn inline_cid_images(html: &str, parts: &[InlinePart]) -> String {
    if parts.is_empty() {
        return html.to_owned();
    }
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel) = lower[cursor..].find(CID_SRC_MARKER) {
        let token_start = cursor + rel; // index of `s` in `src="cid:`
        let value_start = token_start + CID_SRC_MARKER.len(); // first char of the id
        // The id runs to the next double quote that closes the attribute; ammonia escapes
        // any literal `"` in a value to `&quot;`, so the first quote is the real terminator.
        let Some(quote_rel) = html[value_start..].find('"') else {
            break; // malformed (no closing quote): copy the remainder untouched below.
        };
        let value_end = value_start + quote_rel;
        let token_end = value_end + 1; // past the closing quote
        let cid = html[value_start..value_end].trim();

        out.push_str(&html[cursor..token_start]);
        match resolve_image(parts, cid) {
            Some((media_type, bytes)) => {
                out.push_str("src=\"data:");
                out.push_str(&media_type);
                out.push_str(";base64,");
                base64_encode_into(bytes, &mut out);
                out.push('"');
            }
            // Unresolved or non-image: leave the original `src="cid:ID"` token as-is.
            None => out.push_str(&html[token_start..token_end]),
        }
        cursor = token_end;
    }
    out.push_str(&html[cursor..]);
    out
}

/// Resolves a `cid:` token to an inline part's `(image media type, bytes)`, or `None` when
/// no part matches or its media type is not a safe `image/*`. The id match is exact and
/// case-sensitive (RFC 2392 `cid:` references a case-sensitive `Content-ID`).
fn resolve_image<'a>(parts: &'a [InlinePart], cid: &str) -> Option<(String, &'a [u8])> {
    let part = parts.iter().find(|part| part.content_id() == cid)?;
    let media_type = image_media_type(part.media_type())?;
    Some((media_type, part.bytes()))
}

/// The normalised `image/<subtype>` media type to place in a `data:` URI, or `None` if
/// `media_type` is not an image type or carries any character unsafe in a `data:` URL.
///
/// Restricting to `image/*` keeps the rewrite from ever producing an executable/document
/// `data:` URI, and the subtype allowlist (RFC 6838 restricted-name chars) excludes `;`,
/// `,`, and `"`: the characters that could inject a `data:` parameter or break out of the
/// `src` attribute. `image/svg+xml` is allowed: an SVG loaded via `<img>` is rendered
/// non-scripted (the browser disables scripting for image-referenced SVG), and the
/// document CSP carries no `script-src` regardless.
pub(crate) fn image_media_type(media_type: &str) -> Option<String> {
    let lower = media_type.trim().to_ascii_lowercase();
    let subtype = lower.strip_prefix("image/")?;
    let safe = !subtype.is_empty()
        && subtype
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_'));
    safe.then_some(lower)
}

/// Appends the standard (RFC 4648) base64 encoding of `bytes` to `out`. Hand-rolled to keep
/// the core dependency-light; encoding is a fixed 3-byte→4-char mapping with `=` padding.
fn base64_encode_into(bytes: &[u8], out: &mut String) {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    out.reserve(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(b1.unwrap_or(0)) << 8)
            | u32::from(b2.unwrap_or(0));
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(if b1.is_some() {
            ALPHABET[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if b2.is_some() {
            ALPHABET[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
}

/// Reverses [`inline_cid_images`] for the **outbound** (reply/forward) path: rewrites each
/// `data:` image in `html` that came from one of `parts` back to a `cid:` reference using
/// the part's **original** `Content-ID`, and returns the parts that were matched so the
/// caller can re-attach them as inline MIME parts.
///
/// When a reply/forward is sent, the quoted body the editor round-tripped carries the
/// reading view's resolved `data:` images. Sending them as `data:` works in most clients but
/// Outlook's reader blocks `data:` images, so the interoperable form (what Outlook and
/// Thunderbird produce) is a `multipart/related` message with `cid:` parts. The match is
/// exact because the same `Engine::message_inline_parts` fetch produces byte-identical parts
/// at read and send time, so the `data:` URI [`inline_cid_images`] wrote can be reconstructed
/// here and found. The original `Content-ID` is preserved (it can let a spam filter reuse a
/// hash it already scored earlier in the thread, and matches Outlook for Mac exactly).
pub(crate) fn restore_cid_images<'a>(
    html: &str,
    parts: &[&'a InlinePart],
) -> (String, Vec<&'a InlinePart>) {
    let mut out = html.to_owned();
    let mut matched = Vec::new();
    for &part in parts {
        // Reconstruct exactly the `src="data:…"` attribute inline_cid_images would have written;
        // an image whose media type it would have rejected was never inlined, so it can't be here.
        // The match is anchored on the whole `src="…"` value (not the bare `data:` substring) so a
        // part whose base64 is a prefix of another's can't rewrite the middle of that other src.
        let Some(media) = image_media_type(part.media_type()) else {
            continue;
        };
        let mut needle = format!("src=\"data:{media};base64,");
        base64_encode_into(part.bytes(), &mut needle);
        needle.push('"');
        if !out.contains(&needle) {
            continue;
        }
        let mut replacement = String::from("src=\"cid:");
        escape_attr_value(part.content_id(), &mut replacement);
        replacement.push('"');
        out = out.replace(&needle, &replacement);
        matched.push(part);
    }
    (out, matched)
}

/// Escapes the characters that are significant inside a double-quoted HTML attribute value,
/// so a `Content-ID` placed in a `src="cid:…"` can never break out of the attribute. Normal
/// `Content-ID`s (addr-spec tokens) contain none of these, so this is defence in depth.
fn escape_attr_value(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests;
