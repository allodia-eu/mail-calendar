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

/// Whether a link the user clicked in a rendered message should be opened in the OS
/// default browser/handler. The launch policy (a strict scheme allowlist; mail is
/// hostile input) lives in shared Rust so every client decides identically and
/// consistently with what the message sanitiser keeps, only the actual launch is native.
/// `false` means ignore the click. See `docs/rendering-security.md`.
#[uniffi::export]
pub fn should_open_external_link(url: String) -> bool {
    mailcal_app::should_open_external_link(&url)
}
