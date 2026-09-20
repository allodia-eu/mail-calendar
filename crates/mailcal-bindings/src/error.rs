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
    /// The engine could not be opened (or the account id was invalid).
    #[error("engine: {0}")]
    Engine(String),
    /// A composer document could not be parsed, validated, or rendered.
    #[error("composer: {0}")]
    Composer(String),
}
