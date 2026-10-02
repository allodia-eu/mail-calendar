//! Certificate exceptions as the stored account config carries them.
//!
//! A person who could not connect to their own server was shown the certificate it
//! presented and accepted it. What is kept is the SHA-256 of that one certificate,
//! scoped to the one TLS server name, so the exception is worth nothing to any other
//! server and nothing to that server presenting anything else. `engine-tls` enforces
//! it; this module is only the storage form, and `docs/certificate-exceptions.md` is
//! the contract.

use engine_tls::CertificateDer;
use serde::Deserialize;

/// The length of a SHA-256 fingerprint written as hex, without separators.
const FINGERPRINT_HEX_LEN: usize = 64;

/// A server certificate accepted for this account although it did not verify.
///
/// Both fields are as stored: `server_name` matches the account's own
/// `imap.server_name` / `smtp.server_name`, and `sha256` is lowercase hex of the
/// certificate's DER.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CertificateException {
    /// The TLS server name the exception is scoped to.
    pub server_name: String,
    /// SHA-256 of the accepted certificate's DER, lowercase hex.
    pub sha256: String,
}

impl CertificateException {
    /// The exception accepting `certificate` for `server_name`.
    #[must_use]
    pub fn new(server_name: &str, certificate: &CertificateDer<'_>) -> Self {
        Self {
            server_name: server_name.trim().to_lowercase(),
            sha256: hex(&engine_tls::fingerprint(certificate)),
        }
    }

    /// The exception accepting the certificate with fingerprint `sha256` for
    /// `server_name`: the form a client hands back after somebody accepted what it was
    /// shown.
    ///
    /// `sha256` is read in either form, `:`-separated or not and in any case, and stored
    /// canonically. `None` when it is not a SHA-256 fingerprint at all, so nothing
    /// unreadable is ever written to the config.
    #[must_use]
    pub fn accepted(server_name: &str, sha256: &str) -> Option<Self> {
        Some(Self {
            server_name: server_name.trim().to_lowercase(),
            sha256: hex(&parse_fingerprint(sha256)?),
        })
    }

    /// This exception as the table the stored config carries it in.
    pub(crate) fn to_table(&self) -> toml::Table {
        let mut table = toml::Table::new();
        table.insert("server_name".into(), self.server_name.clone().into());
        table.insert("sha256".into(), self.sha256.clone().into());
        table
    }

    /// The engine form, or `None` when `sha256` is not a SHA-256 fingerprint.
    ///
    /// A stored exception nobody can read is dropped rather than refused: the connect it
    /// would have permitted then fails with the certificate error it always did, which
    /// asks the person for the exception again. Refusing to load the account instead
    /// would lock them out of a mailbox over a file they never see.
    pub(crate) fn to_engine(&self) -> Option<engine_tls::CertificateException> {
        Some(engine_tls::CertificateException::from_fingerprint(
            &self.server_name,
            parse_fingerprint(&self.sha256)?,
        ))
    }
}

/// What a server offered that did not verify: enough for somebody to recognise a server
/// they meant to reach, and the fingerprint that accepting it would pin.
///
/// Everything here is the certificate's own claim, which is precisely what failed to
/// verify. It is shown so a person can decide, and nothing else may rest on it. A client
/// that is given one and told to accept it hands the same record back on
/// [`AccountSetup::accepted_certificate`](crate::AccountSetup::accepted_certificate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedCertificate {
    /// The TLS server name that was asked for. The exception would be scoped to it.
    pub server_name: String,
    /// The certificate's SHA-256, uppercase and colon-separated, as every tool displays a
    /// fingerprint and as a person would compare it against their server.
    pub sha256: String,
    /// The name the certificate is **valid for**, which is what a verifier reads: its
    /// `subjectAltName`, or several joined by a comma when it names more than one. The `CN`
    /// only when it carries no `subjectAltName` at all, because nothing checks a `CN` against
    /// the host that was dialled and showing one as the name would show the field the refusal
    /// did not turn on.
    pub subject_common_name: Option<String>,
    /// The organisation the certificate claims to belong to (`O`).
    pub subject_organisation: Option<String>,
    /// The name of whoever issued it. Equal to the subject when it signed itself, which is
    /// how a client can say so.
    pub issuer_common_name: Option<String>,
    /// The organisation that issued it (`O`).
    pub issuer_organisation: Option<String>,
    /// When the certificate claims to become valid, in seconds since the Unix epoch.
    /// `None` when the certificate could not be read at all.
    pub not_before: Option<i64>,
    /// When the certificate claims to expire, in seconds since the Unix epoch. `None` under the
    /// same condition as `not_before`: both are read out of the same parse.
    pub not_after: Option<i64>,
}

impl RejectedCertificate {
    /// Reads what the engine recorded.
    ///
    /// The fingerprint is taken over the bytes, so it is always there; everything else is
    /// read out of the certificate and is absent when those bytes do not parse, which an
    /// unvalidated certificate is entitled not to.
    pub(crate) fn of(rejected: &engine_tls::RejectedCertificate) -> Self {
        let summary = rejected.summary();
        Self {
            server_name: rejected.server_name().to_owned(),
            sha256: format_fingerprint(&rejected.fingerprint()),
            subject_common_name: summary.as_ref().and_then(subject_name),
            subject_organisation: summary
                .as_ref()
                .and_then(|s| s.subject_organization().map(ToOwned::to_owned)),
            issuer_common_name: summary
                .as_ref()
                .and_then(|s| s.issuer_common_name().map(ToOwned::to_owned)),
            issuer_organisation: summary
                .as_ref()
                .and_then(|s| s.issuer_organization().map(ToOwned::to_owned)),
            not_before: summary
                .as_ref()
                .map(engine_tls::CertificateSummary::not_before),
            not_after: summary
                .as_ref()
                .map(engine_tls::CertificateSummary::not_after),
        }
    }

    /// The exception that accepting this certificate would store.
    #[must_use]
    pub fn exception(&self) -> Option<CertificateException> {
        CertificateException::accepted(&self.server_name, &self.sha256)
    }
}

/// The name a person should be shown for a certificate: the names it is valid for, which is
/// what a verifier matches a host against, falling back to the `CN` only when it carries none.
/// A certificate with no `subjectAltName` cannot verify for that reason alone, and its `CN` is
/// then the only thing it says about itself.
fn subject_name(summary: &engine_tls::CertificateSummary) -> Option<String> {
    match summary.subject_names() {
        [] => summary.subject_common_name().map(ToOwned::to_owned),
        names => Some(names.join(", ")),
    }
}

/// A fingerprint as the uppercase, colon-separated groups every tool displays one in
/// (`48:B8:61:…`), which is the form somebody compares against what their server says.
#[must_use]
pub fn format_fingerprint(fingerprint: &[u8; 32]) -> String {
    fingerprint
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// A fingerprint as lowercase hex, the form the stored config carries.
fn hex(fingerprint: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    fingerprint.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Reads a fingerprint back from either form: `:`-separated or not, any case.
fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| *byte != b':' && !byte.is_ascii_whitespace())
        .collect();
    if digits.len() != FINGERPRINT_HEX_LEN {
        return None;
    }
    let mut fingerprint = [0u8; 32];
    let (pairs, _) = digits.as_chunks::<2>();
    for (byte, pair) in fingerprint.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(fingerprint)
}

#[cfg(test)]
mod tests {
    use engine_tls::CertificateDer;

    use super::{CertificateException, format_fingerprint, hex, parse_fingerprint};

    fn certificate() -> CertificateDer<'static> {
        CertificateDer::from(b"a certificate".to_vec())
    }

    #[test]
    fn an_exception_carries_the_certificates_fingerprint() {
        let exception = CertificateException::new("MAIL.Example.com ", &certificate());

        assert_eq!(exception.server_name, "mail.example.com");
        assert_eq!(
            exception.sha256,
            hex(&engine_tls::fingerprint(&certificate()))
        );
        assert_eq!(exception.sha256.len(), 64);
    }

    #[test]
    fn a_stored_exception_reaches_the_engine_in_the_same_shape() {
        let stored = CertificateException::new("mail.example.com", &certificate());
        let engine = stored.to_engine().expect("a well-formed fingerprint");

        assert_eq!(engine.server_name(), "mail.example.com");
        assert_eq!(
            engine.fingerprint(),
            engine_tls::fingerprint(&certificate())
        );
    }

    /// What a client shows is what it may hand back, so the displayed form is accepted
    /// and canonicalised rather than stored as it arrived.
    #[test]
    fn an_acceptance_takes_the_fingerprint_in_the_form_it_was_displayed_in() {
        let displayed = format_fingerprint(&engine_tls::fingerprint(&certificate()));
        let accepted =
            CertificateException::accepted("mail.example.com", &displayed).expect("a fingerprint");

        assert_eq!(
            accepted,
            CertificateException::new("mail.example.com", &certificate())
        );
        assert!(!accepted.sha256.contains(':'));
    }

    #[test]
    fn an_unreadable_fingerprint_is_never_accepted_or_stored() {
        for text in ["", "not hex", &"ab".repeat(31), &"zz".repeat(32)] {
            assert!(
                CertificateException::accepted("mail.example.com", text).is_none(),
                "{text:?} is not a fingerprint"
            );
            assert!(
                parse_fingerprint(text).is_none(),
                "{text:?} is not a fingerprint"
            );
        }
    }

    /// An exception that somehow reached the config unreadable drops out rather than
    /// taking the account down with it.
    #[test]
    fn an_unreadable_stored_exception_drops_rather_than_failing_the_account() {
        let exception = CertificateException {
            server_name: "mail.example.com".to_owned(),
            sha256: "not a fingerprint".to_owned(),
        };
        assert!(exception.to_engine().is_none());
    }

    /// The record a client is shown hands straight back as the exception to store, with
    /// no second spelling of the fingerprint to get wrong.
    #[test]
    fn a_rejected_certificate_offers_the_exception_it_would_become() {
        let rejected = super::RejectedCertificate {
            server_name: "mail.example.com".to_owned(),
            sha256: format_fingerprint(&engine_tls::fingerprint(&certificate())),
            subject_common_name: None,
            subject_organisation: None,
            issuer_common_name: None,
            issuer_organisation: None,
            not_before: None,
            not_after: None,
        };

        assert_eq!(
            rejected.exception(),
            Some(CertificateException::new(
                "mail.example.com",
                &certificate()
            ))
        );
    }

    #[test]
    fn a_displayed_fingerprint_reads_back_as_the_same_bytes() {
        let fingerprint = engine_tls::fingerprint(&certificate());
        let displayed = format_fingerprint(&fingerprint);

        assert_eq!(displayed.matches(':').count(), 31);
        assert_eq!(parse_fingerprint(&displayed), Some(fingerprint));
        assert_eq!(parse_fingerprint(&hex(&fingerprint)), Some(fingerprint));
    }
}
