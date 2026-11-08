//! The connection-facing FFI records: how each account signs in, and the negotiated transport
//! facts behind a diagnostics view. What is wrong with a connection reaches a host as a notice
//! (`docs/notices.md`).

/// Which sign-in an account was connected with; what a host needs to know to re-run it.
///
/// Only the *family*, never an endpoint or an identity, so it is safe to log. A host asks for
/// this when running a notice's sign-in, because the remedy differs: an OAuth account re-runs its
/// provider's browser sign-in, while a password account has to be re-entered in Settings.
#[derive(uniffi::Enum)]
pub enum AccountProvider {
    /// An IMAP/SMTP (+ CalDAV) account authenticated with a stored password.
    Password,
    /// A Microsoft 365 account (Graph, OAuth); re-run `begin_microsoft_login`.
    Microsoft,
    /// A Google account (Gmail + Google Calendar, OAuth); re-run `begin_google_login`.
    Google,
    /// A JMAP account authenticated with a **stored secret** (a password or an API token). There
    /// is no browser flow to re-run, so a host offers the account's own settings.
    Jmap,
    /// A JMAP account connected by **signing in with the provider**: the discovered
    /// RFC 9728 → 8414 → 7591 → PKCE flow. Its sign-in *is* re-runnable, in place: re-run
    /// `begin_jmap_reauth` + `complete_jmap_reauth`, which reuse the grant's persisted endpoints
    /// and registered client id rather than repeating discovery. Split from [`Self::Jmap`]
    /// because the two need opposite remedies and only the account's own config can tell them
    /// apart.
    JmapOauth,
    /// An IMAP account connected by **signing in with its provider**. Signed in again in place
    /// with `begin_account_sign_in` + `complete_account_sign_in`, from its stored grant; it has
    /// no password for a host to ask for.
    ImapOauth,
}

/// The TLS protocol version a live provider negotiated.
#[derive(uniffi::Enum)]
pub enum TlsVersion {
    /// TLS 1.2.
    Tls1_2,
    /// TLS 1.3.
    Tls1_3,
}

/// The HTTP protocol version a live provider last observed.
#[derive(uniffi::Enum)]
pub enum HttpVersion {
    /// HTTP/1.1.
    Http1_1,
    /// HTTP/2.
    Http2,
}

/// Transport facts for one live provider connection.
#[derive(uniffi::Record)]
pub struct ConnectionInfo {
    /// The negotiated TLS version, or `None` when the provider cannot report one.
    pub tls_version: Option<TlsVersion>,
    /// The HTTP version most recently observed, or `None` for non-HTTP providers or before an
    /// HTTP provider has observed a response.
    pub http_version: Option<HttpVersion>,
}
