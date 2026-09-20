//! An account reaches a mail server whose certificate cannot be verified, once its owner has
//! accepted that certificate, and only then.
//!
//! The server here presents a **self-signed CA certificate as its own end-entity
//! certificate**, which is what Proton Mail Bridge's local IMAP listener serves and what
//! `CaUsedAsEndEntity` names. It speaks no IMAP at all: the handshake is what is under test,
//! and it either fails before a byte of IMAP or succeeds and leaves the greeting unanswered,
//! which is the difference these tests read.

use std::{fmt::Write as _, sync::Arc};

use engine_core::ids::AccountId;
use mailcal_account::{AccountConfig, AccountError, CertificateException, load_str};
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::{io::AsyncWriteExt, net::TcpListener};
use tokio_rustls::TlsAcceptor;

/// Starts a TLS server for `127.0.0.1` whose certificate is marked `CA:TRUE`: a certificate
/// authority, served as the leaf, which no trust anchor can rescue. It closes
/// each connection as soon as the handshake finishes, so a client that gets through TLS
/// fails on the missing IMAP greeting rather than hanging.
///
/// Returns the bound port and the certificate it serves.
async fn bridge_shaped_server() -> (u16, rustls::pki_types::CertificateDer<'static>) {
    let key = rcgen::KeyPair::generate().expect("key pair");
    let mut params =
        rcgen::CertificateParams::new(vec!["127.0.0.1".to_owned()]).expect("certificate params");
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "127.0.0.1");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Example Ltd");
    let certificate = params.self_signed(&key).expect("self-signed certificate");
    let der = certificate.der().clone();

    let server = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_no_client_auth()
    .with_single_cert(
        vec![der.clone()],
        PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    )
    .expect("server certificate and key");
    let acceptor = TlsAcceptor::from(Arc::new(server));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local address").port();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if let Ok(mut tls) = acceptor.accept(tcp).await {
                    let _ = tls.shutdown().await;
                }
            });
        }
    });
    (port, der)
}

/// An account pointed at that server, carrying `exceptions`.
fn account(port: u16, exceptions: &[CertificateException]) -> AccountConfig {
    let mut toml = format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"127.0.0.1\"\n\
         username = \"someone@example.com\"\npassword = \"secret\"\n"
    );
    for exception in exceptions {
        let _ = write!(
            toml,
            "\n[[certificate_exception]]\nserver_name = \"{}\"\nsha256 = \"{}\"\n",
            exception.server_name, exception.sha256
        );
    }
    load_str(&toml).expect("a valid account config")
}

async fn connect(config: &AccountConfig) -> Result<(), AccountError> {
    let id = AccountId::try_from("someone@127.0.0.1").expect("account id");
    mailcal_account::connect_mail_providers(config, &id, None)
        .await
        .map(|_| ())
}

/// The reported bug: the connect fails, and it fails *saying which certificate*, with
/// enough of the certificate's own claims for somebody to recognise their own server.
#[tokio::test]
async fn a_server_whose_certificate_does_not_verify_reports_the_certificate() {
    let (port, served) = bridge_shaped_server().await;

    let error = connect(&account(port, &[]))
        .await
        .expect_err("a CA certificate served as the leaf cannot verify");

    let AccountError::CertificateRejected { rejected, reason } = error else {
        panic!("expected a certificate refusal, got: {error}");
    };
    assert!(
        reason.contains("CaUsedAsEndEntity"),
        "the transport's own words are kept for the log: {reason}"
    );
    assert_eq!(rejected.server_name, "127.0.0.1");
    assert_eq!(
        rejected.sha256,
        mailcal_account::format_fingerprint(&engine_tls::fingerprint(&served))
    );
    assert_eq!(rejected.subject_common_name.as_deref(), Some("127.0.0.1"));
    assert_eq!(
        rejected.subject_organisation.as_deref(),
        Some("Example Ltd")
    );
    // Self-signed, so it issued itself.
    assert_eq!(rejected.issuer_common_name.as_deref(), Some("127.0.0.1"));
    assert!(rejected.not_before < rejected.not_after);
}

/// Accepting that certificate gets the account through TLS. The connect still fails,
/// because this server speaks no IMAP, but it fails on IMAP rather than on the certificate,
/// which is the whole of what the exception changes.
#[tokio::test]
async fn the_accepted_certificate_gets_the_account_through_tls() {
    let (port, _) = bridge_shaped_server().await;

    let error = connect(&account(port, &[]))
        .await
        .expect_err("refused without an exception");
    let AccountError::CertificateRejected { rejected, .. } = error else {
        panic!("expected a certificate refusal");
    };
    let accepted = rejected
        .exception()
        .expect("the refusal offers its exception");

    let error = connect(&account(port, &[accepted]))
        .await
        .expect_err("the server answers no IMAP greeting");
    assert!(
        matches!(error, AccountError::Imap(_)),
        "TLS was settled and IMAP is what failed, got: {error}"
    );
}

/// An exception is a pin, not a setting that turns verification off for a host: the same
/// server presenting a different certificate is refused exactly as it was. This is the
/// test that would fail if an exception ever became "trust whatever this host sends".
#[tokio::test]
async fn an_exception_does_not_cover_a_second_certificate_from_the_same_server() {
    let (_, elsewhere) = bridge_shaped_server().await;
    let (port, _) = bridge_shaped_server().await;

    let stale = CertificateException::new("127.0.0.1", &elsewhere);
    let error = connect(&account(port, &[stale]))
        .await
        .expect_err("a certificate nobody accepted is still refused");

    assert!(
        matches!(error, AccountError::CertificateRejected { .. }),
        "expected the refusal to stand, got: {error}"
    );
}

/// An exception scoped to another server does nothing here, so one accepted for a mail
/// host cannot quietly cover a calendar host that happens to be misconfigured too.
#[tokio::test]
async fn an_exception_for_another_server_does_not_cover_this_one() {
    let (port, served) = bridge_shaped_server().await;

    let elsewhere = CertificateException::new("mail.example.com", &served);
    let error = connect(&account(port, &[elsewhere]))
        .await
        .expect_err("an exception for another server admits nothing here");

    assert!(
        matches!(error, AccountError::CertificateRejected { .. }),
        "expected the refusal to stand, got: {error}"
    );
}
