//! The error one account's providers fail to build or connect with.
//!
//! One taxonomy for every family, because a caller acts on *what went wrong*, never on which
//! provider surfaced it. Two variants are decided here rather than re-derived downstream from a
//! rendered string: a credential the server **refused**, and a certificate it presented that did
//! not verify. Each is a different thing for a caller to do, and neither survives being turned
//! into a message.

use std::fmt;

use provider_imap::ImapError;

use crate::{FailureClass, RejectedCertificate};

/// An error building or connecting an account's providers.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    /// The mailbox id was not valid.
    #[error("invalid mailbox id: {0}")]
    Mailbox(String),
    /// Listing the account's folders failed (needed to find the Sent mailbox).
    #[error("listing mailboxes: {0}")]
    MailboxList(String),
    /// The IMAP connection or login failed.
    #[error("imap: {0}")]
    Imap(#[from] provider_imap::ImapError),
    /// The server presented a certificate that did not verify, and this is the certificate
    /// it presented.
    ///
    /// Apart from every other connect failure because it is the one a person can settle
    /// without changing anything about the account: they are shown what the server
    /// offered and may accept that one certificate for that one server
    /// (`docs/certificate-exceptions.md`). Boxed because a certificate is kilobytes
    /// while every other variant here is a string.
    #[error("certificate not verified: {reason}")]
    CertificateRejected {
        /// What the transport said, kept for the diagnostic log.
        reason: String,
        /// The server name asked for, and the certificate it answered with.
        rejected: Box<RejectedCertificate>,
    },
    /// Building the account's shared TLS policy failed.
    #[error("tls: {0}")]
    Tls(#[from] engine_tls::TlsError),
    /// A Microsoft Graph call failed (the `/me` address lookup, a token refresh, or a
    /// folder-list/connect error) while building a Microsoft account's providers.
    #[error("graph: {0}")]
    Graph(String),
    /// The server refused the account's **stored credential** outright: an OAuth refresh token
    /// that is revoked or expired (`invalid_grant`, `AADSTS700082`) and so mints no access token,
    /// or a password/API token answered with `[AUTHENTICATIONFAILED]` / `401`. Distinct from every
    /// per-family variant because it is the one failure a retry cannot fix and an outage badge
    /// misdescribes (the server *was* reached) so a caller prompts "sign in again"
    /// (`docs/provider-oauth.md` rule 12).
    ///
    /// Deliberately not named for a token: every family's refusal maps here, and which kind of
    /// credential the server refused changes nothing a caller does about it.
    #[error("sign-in rejected: {0}")]
    SigninRejected(String),
    /// A Google (Gmail/Calendar) call failed (the profile address lookup, a token refresh, or
    /// a calendar-list/connect error) while building or driving a Google account's providers.
    /// The Google parallel of [`AccountError::Graph`].
    #[error("google: {0}")]
    Google(String),
    /// The Graph **calendar** probe (`GET /me/calendars`) was refused with a `403`; the
    /// account's OAuth grant lacks the `Calendars.ReadWrite` scope (it was connected before
    /// calendar support, or consent was revoked). Distinct from a transient
    /// [`AccountError::Graph`] so a caller can prompt the user to **re-authenticate to grant
    /// calendar access** rather than badge a generic outage: mail is unaffected.
    #[error("calendar access denied (re-authentication needed): {0}")]
    CalendarAccessDenied(String),
    /// A JMAP call failed (session discovery, connect, or a sync/submission error)
    /// while building or driving a JMAP account's provider.
    #[error("jmap: {0}")]
    Jmap(String),
    /// Opening an IMAP `IDLE` watch failed (connect/login error, or the server does not
    /// advertise `IDLE`: the host falls back to polling).
    #[error("imap watch: {0}")]
    Watch(String),
    /// CalDAV was requested but the config has no `[caldav]` section.
    #[error("no caldav endpoint configured")]
    NoCalDav,
    /// The CalDAV connection or discovery failed.
    #[error("caldav: {0}")]
    CalDav(#[from] provider_caldav::CalDavError),
    /// Listing the account's calendars failed (no `calendar` was configured, so the
    /// connection had to discover one).
    #[error("caldav calendar discovery: {0}")]
    CalDavDiscovery(String),
    /// No calendar collection was discovered, and the config named none to bind to.
    #[error("no caldav calendar discovered (configure `calendar` in [caldav])")]
    NoCalendarDiscovered,
    /// Building a calendar event-write failed (a bad uid, time, or href).
    #[error("calendar write: {0}")]
    CalendarWrite(String),
    /// Building a contact write failed: the edit named nothing to file the card under, or
    /// carried a value that is not an email address.
    ///
    /// The message states the *shape* that was wrong and never quotes the value: a contact's
    /// values are content, and this reaches the diagnostic log (`docs/logging.md`).
    #[error("contact write: {0}")]
    ContactWrite(String),
}

impl AccountError {
    /// Re-reads a connect failure as a certificate refusal when `tls` recorded one.
    ///
    /// The transport reports a refused certificate as a transport error like any other, so
    /// *which* certificate was refused exists only in the TLS config that refused it. One
    /// config is built per connect, so what it holds belongs to this failure.
    pub(crate) fn over_tls(self, tls: &engine_tls::TlsClientConfig) -> Self {
        match tls.rejected() {
            Some(rejected) => Self::CertificateRejected {
                reason: self.to_string(),
                rejected: Box::new(RejectedCertificate::of(&rejected)),
            },
            None => self,
        }
    }

    /// Wraps a calendar-discovery failure, keeping its message (the source types
    /// differ: a provider error or an invalid placeholder id).
    pub(crate) fn caldav_discovery(err: impl fmt::Display) -> Self {
        Self::CalDavDiscovery(err.to_string())
    }

    /// The verdict for the login that **first** presents an IMAP account's password in a dial;
    /// the only one whose refusal can mean the password itself is no good. An
    /// [authentication-class](FailureClass::Authentication) refusal becomes
    /// [`Self::SigninRejected`]; anything else keeps [`Self::Imap`].
    ///
    /// A later connection of the same dial deliberately does **not** come through here. The same
    /// password authenticated seconds earlier, so a refusal there is the server contradicting
    /// itself, and prompting for a new sign-in over it is the false prompt
    /// `docs/provider-oauth.md` rule 12 forbids; servers do refuse a valid credential.
    pub(crate) fn from_first_imap_login(err: ImapError) -> Self {
        if err.failure_class() == FailureClass::Authentication {
            Self::SigninRejected(err.to_string())
        } else {
            Self::Imap(err)
        }
    }

    /// The verdict for a JMAP connect, which presents the credential on **every** attempt (session
    /// discovery authenticates, so there is no first-then-folders sequence to corroborate against):
    /// an [authentication-class](FailureClass::Authentication) refusal: a `401` to a password, an
    /// API token or a bearer; becomes [`Self::SigninRejected`], anything else [`Self::Jmap`].
    pub(crate) fn from_jmap_connect(err: &provider_jmap::JmapError) -> Self {
        if err.failure_class() == FailureClass::Authentication {
            Self::SigninRejected(err.to_string())
        } else {
            Self::Jmap(err.to_string())
        }
    }
}
