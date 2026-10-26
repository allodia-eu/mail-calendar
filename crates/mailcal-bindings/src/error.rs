//! FFI error type shared by every binding module.

use crate::RejectedCertificate;

/// An error building or driving a real account-backed [`crate::MailcalApp`]. Carries a
/// message rather than the source type so it crosses the FFI as a plain error a host can
/// surface.
#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum MailcalError {
    /// The account config could not be loaded or parsed.
    #[error("config: {0}")]
    Config(String),
    /// Connecting or logging in to the provider failed.
    #[error("connect: {0}")]
    Connect(String),
    /// The server presented a certificate that could not be verified.
    ///
    /// Its own outcome rather than a [`MailcalError::Connect`] because it is the one
    /// connect failure the person in front of the screen can settle: a client shows
    /// `certificate` and, if they accept it, hands the same record back on
    /// [`AccountSetup::accepted_certificate`](crate::AccountSetup::accepted_certificate)
    /// and connects again ([`docs/certificate-exceptions.md`]).
    ///
    /// The display carries `reason` only, so a client that renders the error as text says
    /// what went wrong without the certificate's details landing in a log line.
    ///
    /// [`docs/certificate-exceptions.md`]: https://github.com/allodia-eu/mail-calendar/blob/main/docs/certificate-exceptions.md
    #[error("certificate: {reason}")]
    CertificateRejected {
        /// What the transport said, for the diagnostic log.
        reason: String,
        /// What the server presented, for the person deciding.
        certificate: RejectedCertificate,
    },
    /// The server answered and refused the sign-in: a wrong password or API token, or a grant it
    /// no longer accepts.
    ///
    /// Its own outcome rather than a [`MailcalError::Connect`] so a client can say so in the
    /// person's language: the server's reply is a status and a body meant for a developer.
    /// `reason` is that reply, for the diagnostic log, and the display carries it alone.
    #[error("sign-in refused: {reason}")]
    SignInRefused {
        /// What the server said, for the diagnostic log.
        reason: String,
    },
    /// The account being added is already set up: an account's id is its mailbox (login and
    /// server), not how it signs in, so a second add for the same mailbox would replace the first.
    ///
    /// Its own outcome so a client can say so in the person's language and offer what to do
    /// instead, keyed by `account_id`. The display carries no address, because it reaches logs.
    #[error("that account is already set up")]
    AlreadySetUp {
        /// The id of the account already set up.
        account_id: String,
    },
    /// A sign-in or setup came back as a different mailbox than the account it was for, and was
    /// discarded: a `login_hint` targets an address, it does not pin one.
    ///
    /// Its own outcome so a client can say so in the person's language: nothing is wrong with
    /// the server or the credential, only with which account was chosen in the browser.
    #[error("that sign-in is for a different account")]
    DifferentAccount,
    /// The browser came back from another authorization request than the one this sign-in
    /// opened, and its code was discarded unused.
    ///
    /// Its own outcome because its usual cause is innocent and a person can settle it: the
    /// provider's login page returned, once the person had signed in there, to an older request
    /// the same browser left unfinished. Signing in again goes straight through, since the
    /// browser is now signed in at the provider (`docs/provider-oauth.md` rule 3).
    #[error("the browser returned from another sign-in")]
    StaleSignIn,
    /// The calendar or address book a server of its own was to keep did not connect, so nothing
    /// changed (`docs/accounts.md` rule 18).
    ///
    /// Its own outcome so a client can say, in the person's language, which use failed and why.
    /// A refused password is [`MailcalError::SignInRefused`] instead.
    #[error("{capability:?} did not connect: {problem:?}")]
    UseNotConnected {
        /// The use that did not connect.
        capability: crate::AccountCapability,
        /// Why not.
        problem: crate::UseProblem,
    },
    /// The engine could not be opened (or the account id was invalid).
    #[error("engine: {0}")]
    Engine(String),
    /// A composer document could not be parsed, validated, or rendered.
    #[error("composer: {0}")]
    Composer(String),
}

/// What a failed token exchange is to a client: the browser returning from another sign-in has a
/// remedy of its own, and anything else is a connect failure carrying its detail for the log.
pub(crate) fn exchange_error(error: &mailcal_oauth::OAuthError) -> MailcalError {
    match error {
        mailcal_oauth::OAuthError::StateMismatch => MailcalError::StaleSignIn,
        other => MailcalError::Connect(other.to_string()),
    }
}
