//! The connection-facing FFI records: what the app can and can't currently reach, why, and how
//! each account signs in.
//!
//! Split out of `records.rs` for the 500-line limit, and cohesive on its own: these are the types
//! a host reads to answer "is anything wrong, and what should I offer the user about it?"; the
//! device-offline flag, the per-account outages and re-consent prompts, the sign-in family behind
//! a reconnect button, and the negotiated transport facts behind a diagnostics view.

/// An immutable connectivity snapshot for a host to render: the device-offline flag and the
/// accounts that couldn't reach their server.
#[derive(uniffi::Record)]
pub struct ConnectivitySnapshot {
    /// Whether the device has no network connectivity (per the OS reachability API): a host
    /// shows a global offline banner; the core suppresses syncs until it clears.
    pub offline: bool,
    /// Ids of accounts whose last sync couldn't reach their server **while online**: a host
    /// badges these in the switcher. Always empty while `offline` (then it's the device).
    pub unreachable_accounts: Vec<String>,
    /// Ids of accounts whose calendar is withheld because their OAuth grant lacks the calendar
    /// scope (a Microsoft account connected before calendar support, or with revoked consent).
    /// **Mail is unaffected.** A host shows a "reconnect to enable calendar" prompt whose action
    /// re-runs the account's Microsoft sign-in (`begin_microsoft_login` with the account's
    /// address as `login_hint`, then `complete_microsoft_login`, which upgrades the existing
    /// account's token in place). Not suppressed while offline (a standing permission gap).
    pub calendar_reauth_accounts: Vec<String>,
    /// Ids of accounts whose mail **write/send** is withheld because their OAuth grant lacks the
    /// `Mail.ReadWrite` / `Mail.Send` scopes (a Microsoft account connected before those scopes,
    /// or with revoked consent); surfaced when a send or a mail action (mark-read/flag/move/
    /// delete) is refused with `403 ErrorAccessDenied`. Mail **reading** is unaffected. A host
    /// shows a "reconnect to send and manage mail" prompt whose action is the **same** OAuth
    /// re-run as `calendar_reauth_accounts` (`begin_microsoft_login` with the address as
    /// `login_hint`, then `complete_microsoft_login`); one re-consent re-grants the whole scope
    /// set, clearing both. Not suppressed while offline (a standing permission gap).
    pub mail_reauth_accounts: Vec<String>,
    /// Ids of accounts whose stored sign-in is **dead**: the OAuth grant expired or was revoked
    /// (Google `invalid_grant`, a Microsoft `AADSTS700082`), or a password account's credential is
    /// refused. **Nothing** about the account syncs until the user signs in again, and a retry
    /// never helps: the server was reached and answered. A host shows a "your sign-in expired;
    /// reconnect" prompt whose action re-runs that account's sign-in, ask
    /// `MailcalApp::account_provider` which one to launch, since this can hit a Microsoft, Google,
    /// OAuth JMAP, or password account. An account listed here is **absent** from
    /// `unreachable_accounts`, so
    /// the two never contradict each other; like the reauth prompts it is not suppressed while
    /// offline.
    pub signin_expired_accounts: Vec<String>,
}

/// Which sign-in an account was connected with; what a host needs to know to re-run it.
///
/// Only the *family*, never an endpoint or an identity, so it is safe to log. A host asks for
/// this when rendering a reconnect prompt (`ConnectivitySnapshot::signin_expired_accounts`),
/// because the remedy differs: an OAuth account re-runs its provider's browser sign-in, while a
/// password account has to be re-entered in Settings.
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
