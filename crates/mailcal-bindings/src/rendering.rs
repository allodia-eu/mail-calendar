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

/// One labelled line of a printed message's header: the client's localised label and what the
/// message says for it. A line with an empty value is left out, so a client passes every line it
/// draws in its reading header without filtering.
#[derive(uniffi::Record)]
pub struct PrintHeaderLine {
    /// The line's name as the client shows it (`From`, `Van`).
    pub label: String,
    /// The value, as the reading header shows it.
    pub value: String,
}

/// The document a client prints for one message: the subject and `lines` as escaped text above
/// the body, inside the same strict-CSP document [`render_message_html`] builds. `html` is the
/// snapshot's sanitised fragment and wins over `plain`; `load_remote_images` is the reader's choice
/// for this message, so a print never loads what reading did not. The host loads it into a web view
/// with the reading view's gates and hands that to the platform's print dialog
/// (`docs/reading-actions.md`).
#[uniffi::export]
pub fn render_message_print_html(
    subject: String,
    lines: Vec<PrintHeaderLine>,
    html: Option<String>,
    plain: Option<String>,
    load_remote_images: bool,
) -> String {
    let lines: Vec<_> = lines
        .into_iter()
        .map(|line| mailcal_app::PrintHeaderLine {
            label: line.label,
            value: line.value,
        })
        .collect();
    mailcal_app::render_print_document(
        &subject,
        &lines,
        html.as_deref(),
        plain.as_deref(),
        load_remote_images,
    )
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
