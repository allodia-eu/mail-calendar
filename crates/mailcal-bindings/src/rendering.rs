//! Shared message-rendering helpers exposed over UniFFI.

/// Wraps a [`crate::ReadingSnapshot::html`] body fragment in a complete, strict-CSP HTML
/// document for a host to load into its WebView; shared in Rust so the security boundary
/// and base styling are identical across every client. `load_remote_images` reflects the
/// user's per-message choice: `false` (default) blocks all remote images, `true` loads
/// them after the user accepts the "load remote images" prompt. The host still renders
/// with JavaScript disabled and navigation blocked.
#[uniffi::export]
pub fn render_message_html(html: String, load_remote_images: bool) -> String {
    mailcal_app::render_document(&html, load_remote_images)
}

/// The canvas the reading pane's body area is drawn on, `#rrggbb`: the same page
/// [`render_message_html`] gives the document, so the client's half and the document's half
/// are one colour rather than two whites that drift apart.
///
/// It is the canvas in **both** themes, and the client paints it for the whole of an open:
/// while the body is still resolving, behind a plain-text body, behind a load error. A host
/// that leaves the waiting gap transparent punches a hole in the page: on a dark theme the
/// body area goes white, black, white on every message the user opens, which reads as a
/// flicker rather than as the message opening. `docs/sync-progress.md` binds every client to
/// this.
#[derive(uniffi::Record)]
pub struct MessageCanvas {
    /// The page's fill.
    pub background: String,
    /// The text colour that stays legible on that fill.
    pub foreground: String,
}

/// The canvas a message is drawn on; see [`MessageCanvas`].
#[uniffi::export]
pub fn message_canvas() -> MessageCanvas {
    MessageCanvas {
        background: mailcal_app::MESSAGE_CANVAS.background.to_owned(),
        foreground: mailcal_app::MESSAGE_CANVAS.foreground.to_owned(),
    }
}

/// Whether a link the user clicked in a rendered message should be opened in the OS
/// default browser/handler. The launch policy (a strict scheme allowlist; mail is
/// hostile input) lives in shared Rust so every client decides identically and
/// consistently with what the message sanitiser keeps, only the actual launch is native.
/// `false` means ignore the click. See `docs/rendering-security.md`.
#[uniffi::export]
pub fn should_open_external_link(url: String) -> bool {
    mailcal_app::should_open_external_link(&url)
}

/// A run of text a client draws natively, and where it leads when it is an address.
///
/// Built by the core, never by the client: the text is appended to the client's own rich-text
/// value **as text**, and `link` is the only thing that makes a run a link, so no markup or
/// markdown parser ever sees sender content (`docs/rendering-security.md`, gate 8).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct LinkedText {
    /// The text exactly as written.
    pub text: String,
    /// The target to open, when this run is an address. Already on the
    /// [`should_open_external_link`] allowlist; a client still asks that gate before it launches.
    pub link: Option<String>,
}

/// `text` split into plain runs and the web and mail addresses written in it, covering all of it,
/// for a client that draws sender text natively: a plain-text body, an event's notes, an
/// invitation's description. The same finder links the addresses in an HTML body, so a link is a
/// link on every surface. Empty text gives no runs.
#[uniffi::export]
pub fn linked_text(text: String) -> Vec<LinkedText> {
    mailcal_app::link_segments(&text)
        .into_iter()
        .map(|segment| LinkedText {
            text: segment.text,
            link: segment.link,
        })
        .collect()
}
