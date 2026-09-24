//! An account's first `LOGIN` is the only one that can prove its password wrong, so what that
//! refusal is read as decides whether the user is asked to sign in again.
//!
//! The server here greets, reads the `LOGIN`, and refuses it with whatever answer the test
//! gives. It sits behind a self-signed certificate the account has accepted, so the connect
//! runs through the real TLS and IMAP path rather than a fake of either.

use std::sync::Arc;

use engine_core::{error::FailureClass, ids::AccountId};
use mailcal_account::{AccountConfig, AccountError, CertificateException, load_str};
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

/// Starts an IMAP server on `127.0.0.1` that answers every `LOGIN` with `refusal`, and returns
/// an account pointed at it that has accepted its certificate.
async fn server_refusing_login_with(refusal: &'static str) -> AccountConfig {
    let key = rcgen::KeyPair::generate().expect("key pair");
    let params =
        rcgen::CertificateParams::new(vec!["127.0.0.1".to_owned()]).expect("certificate params");
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
                let Ok(tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut tls = BufReader::new(tls);
                let _ = tls
                    .write_all(b"* OK [CAPABILITY IMAP4rev1] ready\r\n")
                    .await;
                let mut login = String::new();
                if tls.read_line(&mut login).await.is_err() {
                    return;
                }
                let tag = login.split_whitespace().next().unwrap_or("*");
                let _ = tls
                    .write_all(format!("{tag} {refusal}\r\n").as_bytes())
                    .await;
                let _ = tls.shutdown().await;
            });
        }
    });

    let accepted = CertificateException::new("127.0.0.1", &der);
    load_str(&format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"127.0.0.1\"\n\
         username = \"someone@example.com\"\npassword = \"secret\"\n\
         \n[[certificate_exception]]\nserver_name = \"{}\"\nsha256 = \"{}\"\n",
        accepted.server_name, accepted.sha256
    ))
    .expect("a valid account config")
}

async fn first_login(config: &AccountConfig) -> AccountError {
    let id = AccountId::try_from("someone@127.0.0.1").expect("account id");
    let connections = mailcal_account::ImapConnections::new();
    match mailcal_account::connect_mail_providers(&connections, config, &id).await {
        Ok(_) => panic!("the server refuses every LOGIN"),
        Err(err) => err,
    }
}

/// Yahoo's answer, verbatim, to a sixth session opened seconds after five that authenticated
/// with the same password. Read as a refused credential, it tells the user their password is
/// wrong and asks for it again, which cannot help.
#[tokio::test]
async fn a_first_login_refused_for_a_limit_is_not_a_rejected_signin() {
    let config = server_refusing_login_with("NO [LIMIT] LOGIN Rate limit hit.").await;

    let error = first_login(&config).await;
    let AccountError::Imap(imap) = &error else {
        panic!("a limit was read as something other than an IMAP failure: {error}");
    };
    assert_eq!(imap.failure_class(), FailureClass::RateLimited, "{error}");
}

/// The other direction, so the test above cannot pass because the fake failed before `LOGIN`:
/// the same server refusing the credential still raises the sign-in prompt.
#[tokio::test]
async fn a_first_login_refused_for_the_credential_is_a_rejected_signin() {
    let config =
        server_refusing_login_with("NO [AUTHENTICATIONFAILED] Authentication failed.").await;

    let error = first_login(&config).await;
    assert!(
        matches!(error, AccountError::SigninRejected(_)),
        "a refused credential was not read as one: {error}"
    );
}
