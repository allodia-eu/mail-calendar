//! The error one account's providers fail to build or connect with.
//!
//! One taxonomy for every family, because a caller acts on *what went wrong*, never on which
//! provider surfaced it. Two variants are decided here rather than re-derived downstream from a
//! rendered string: a credential the server **refused**, and a certificate it presented that did
//! not verify. Each is a different thing for a caller to do, and neither survives being turned
//! into a message.

use engine_core::error::FailureClass;
use provider_imap::ImapError;

use crate::RejectedCertificate;

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
    /// No connection is free to watch a folder: syncing needs every one left, because the
    /// account already watches as many folders as its budget allows or the server is taking
    /// fewer connections from it for now. A state the host waits out, not a fault: one comes
    /// free when a watch ends or the server's limit lifts.
    #[error("no connection is free to watch a folder; syncing needs every one left")]
    NoWatchConnection,
    /// CalDAV was requested but the config has no `[caldav]` section.
    #[error("no caldav endpoint configured")]
    NoCalDav,
    /// The account has no mailbox: it is used for its calendar or its contacts alone, so there
    /// is no IMAP endpoint to connect to.
    #[error("no imap endpoint configured")]
    NoImap,
    /// The CalDAV connection or discovery failed.
    #[error("caldav: {0}")]
    CalDav(#[from] provider_caldav::CalDavError),
    /// The CardDAV connection or discovery failed. The adapter shares CalDAV's error type, so
    /// this variant is what says the address book was the server dialled.
    #[error("carddav: {0}")]
    CardDav(provider_caldav::CalDavError),
    /// DAV discovery could not be prepared or completed.
    #[error("dav discovery: {0}")]
    CalDavDiscovery(String),
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
    /// The account's sign-in does not reach this calendar or contacts server
    /// (`docs/mail-oauth.md` rule 10): its grant asked for no scope for the use, or the server's
    /// host is not one its issuer's tokens are for. Nothing was sent; the use needs a sign-in of
    /// its own. Carries `calendar` or `contacts`.
    #[error("the account's sign-in does not reach its {0} server")]
    DavNeedsOwnSignIn(&'static str),
    /// The server is reached only over plaintext, so the engine sends it no sign-in: the account's
    /// address for it is `http://` off this device, or the server led there (a redirect, or a
    /// discovery step naming a plaintext address) from one the engine accepts. Nothing was sent.
    /// No new password or sign-in clears it (`docs/provider-oauth.md` rule 12). Carries the
    /// engine's words, for the diagnostic log.
    #[error("the server is not reached securely, so no sign-in is sent to it: {detail}")]
    InsecureServer {
        /// Which of the two, as the engine says it.
        cause: crate::InsecureCause,
        /// The engine's words, for the diagnostic log.
        detail: String,
    },
    /// The account names no credential this connect can present: it signs in with OAuth but
    /// was connected without its token source, or stores neither a password nor a grant. A
    /// stored config makes both impossible and a hand-edited one does not.
    #[error("no usable credential: {0}")]
    MissingCredential(&'static str),
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

    /// The verdict for the login that **first** presents an IMAP account's password in a dial;
    /// the only one whose refusal can mean the password itself is no good. A refused sign-in
    /// becomes [`Self::SigninRejected`] and a server reached only over plaintext
    /// [`Self::InsecureServer`] ([`Verdict::of`]); anything else keeps [`Self::Imap`].
    ///
    /// A later connection of the same dial deliberately does **not** come through here. The same
    /// password authenticated seconds earlier, so a refusal there is the server contradicting
    /// itself, and prompting for a new sign-in over it is the false prompt
    /// `docs/provider-oauth.md` rule 12 forbids; servers do refuse a valid credential.
    pub(crate) fn from_first_imap_login(err: ImapError, signs_in: SignsIn) -> Self {
        let credential = match &err {
            ImapError::Credential(failure) => Some(failure),
            _ => None,
        };
        match Verdict::of(err.failure_class(), credential, signs_in) {
            Verdict::Refused => Self::SigninRejected(err.to_string()),
            Verdict::Insecure(cause) => Self::InsecureServer {
                cause,
                detail: err.to_string(),
            },
            Verdict::NotPermitted | Verdict::Other => Self::Imap(err),
        }
    }

    /// The verdict for a JMAP connect, which presents the credential on **every** attempt (session
    /// discovery authenticates, so there is no first-then-folders sequence to corroborate against):
    /// a refused sign-in, such as a `401` to a password, an API token or a bearer, becomes
    /// [`Self::SigninRejected`] and a server reached only over plaintext [`Self::InsecureServer`]
    /// ([`Verdict::of`]); anything else [`Self::Jmap`].
    pub(crate) fn from_jmap_connect(err: &provider_jmap::JmapError, signs_in: SignsIn) -> Self {
        let credential = match err {
            provider_jmap::JmapError::Credential(failure) => Some(failure),
            _ => None,
        };
        match Verdict::of(err.failure_class(), credential, signs_in) {
            Verdict::Refused => Self::SigninRejected(err.to_string()),
            Verdict::Insecure(cause) => Self::InsecureServer {
                cause,
                detail: err.to_string(),
            },
            Verdict::NotPermitted | Verdict::Other => Self::Jmap(err.to_string()),
        }
    }

    /// Whether this is a DAV server saying the start it was given is **gone**: a `404`, `410` or
    /// `403`, or an answer that names no home (the engine's `Permanent` class). Distinct from a
    /// server that could not be reached (a timeout, DNS, a `5xx`, `429`), which changes nothing:
    /// only a gone start is worth finding the servers again for (RFC 6764 §6). A server that
    /// asks only for an authentication method the credential cannot use is not gone either:
    /// it answered, and looking it up again would find it where it is. Nor is a credential that
    /// could not be had, such as one discovery would have taken to a host it may not go to: no
    /// server was asked.
    #[must_use]
    pub fn is_gone(&self) -> bool {
        use provider_caldav::CalDavError;
        matches!(
            self,
            Self::CalDav(err) | Self::CardDav(err)
                if err.failure_class() == FailureClass::Permanent
                    && !matches!(
                        err,
                        CalDavError::UnsupportedAuthentication(_) | CalDavError::Credential(_)
                    )
        )
    }

    /// The account's sign-in does not reach its server for `dav_use`
    /// ([`Self::DavNeedsOwnSignIn`]).
    pub(crate) fn dav_needs_own_sign_in(dav_use: mailcal_oauth::DavUse) -> Self {
        Self::DavNeedsOwnSignIn(match dav_use {
            mailcal_oauth::DavUse::Calendars => "calendar",
            mailcal_oauth::DavUse::Contacts => "contacts",
        })
    }

    /// The verdict for the connect that first presents a DAV endpoint's credential
    /// ([`Verdict::of`]): a refused sign-in becomes [`Self::SigninRejected`], a server reached
    /// only over plaintext [`Self::InsecureServer`], and an origin the account's grant may not
    /// take its token to, for `grant`, [`Self::DavNeedsOwnSignIn`] (`docs/mail-oauth.md` rule
    /// 10); anything else is what `failed` makes of it ([`Self::CalDav`] or [`Self::CardDav`]).
    ///
    /// A rejected sign-in decides something only for an account without mail, whose calendar or
    /// contacts are what say whether its sign-in still works (`docs/accounts.md` rule 7). Beside a
    /// mailbox the mailbox decides, and a DAV failure only leaves the calendar or contacts empty.
    /// The requests that follow the connect convert the ordinary way, for the reason
    /// [`Self::from_first_imap_login`] gives.
    pub(crate) fn from_first_dav_connect(
        err: provider_caldav::CalDavError,
        failed: fn(provider_caldav::CalDavError) -> Self,
        grant: Option<mailcal_oauth::DavUse>,
    ) -> Self {
        let credential = match &err {
            provider_caldav::CalDavError::Credential(failure) => Some(failure),
            _ => None,
        };
        let signs_in = SignsIn::grant_if(grant.is_some());
        match (
            Verdict::of(err.failure_class(), credential, signs_in),
            grant,
        ) {
            (Verdict::Refused, _) => Self::SigninRejected(err.to_string()),
            (Verdict::Insecure(cause), _) => Self::InsecureServer {
                cause,
                detail: err.to_string(),
            },
            (Verdict::NotPermitted, Some(dav_use)) => Self::dav_needs_own_sign_in(dav_use),
            _ => failed(err),
        }
    }
}

/// What an account signs in with, which decides what a credential the engine will not present
/// means on its first connect ([`Verdict::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignsIn {
    /// A password or API token the account stores.
    WithStoredSecret,
    /// A token minted from the account's grant.
    WithGrant,
}

impl SignsIn {
    /// [`Self::WithGrant`] for an account that stores a grant, else [`Self::WithStoredSecret`].
    pub(crate) const fn grant_if(has_grant: bool) -> Self {
        if has_grant {
            Self::WithGrant
        } else {
            Self::WithStoredSecret
        }
    }
}

/// What a first connect's failure says about the account's sign-in, decided here for IMAP, JMAP,
/// CalDAV and CardDAV alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// The sign-in is refused, and the person signs in again: the server refused the credential,
    /// or the account stores a secret the protocol cannot carry (a Basic username holding `:`, a
    /// byte an IMAP `LOGIN` or a header has no room for), which never goes out as stored.
    Refused,
    /// The server is reached only over plaintext, so no sign-in is sent to it, whatever the
    /// account signs in with: the address is plaintext, or the server led there, as the engine
    /// says which.
    Insecure(crate::InsecureCause),
    /// The account's grant may not take its token to the server, by the issuer's rule.
    NotPermitted,
    /// Anything else: the family's own failure.
    Other,
}

impl Verdict {
    /// The verdict on a first connect that failed with `class`, and with `credential` when the
    /// adapter reports a credential that could not be had (`ProviderError::credential_failure`
    /// says why), for an account that `signs_in` as it does. A source that failed keeps its
    /// class's meaning: a grant that is gone is `Authentication`, a refused sign-in.
    fn of(
        class: FailureClass,
        credential: Option<&engine_provider::ProviderError>,
        signs_in: SignsIn,
    ) -> Self {
        use engine_provider::CredentialFailureKind as Kind;
        if class == FailureClass::Authentication {
            return Self::Refused;
        }
        match credential.and_then(engine_provider::ProviderError::credential_failure) {
            Some(Kind::Insecure) => Self::Insecure(crate::InsecureCause::PlaintextAddress),
            Some(Kind::LedToInsecure) => Self::Insecure(crate::InsecureCause::LedToPlaintext),
            Some(Kind::Form) if signs_in == SignsIn::WithStoredSecret => Self::Refused,
            Some(Kind::NotPermitted) if signs_in == SignsIn::WithGrant => Self::NotPermitted,
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
#[path = "error_credential_tests.rs"]
mod credential_tests;
