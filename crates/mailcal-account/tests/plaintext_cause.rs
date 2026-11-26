//! Why a server is reached only over plaintext is the engine's to say: the account's own address
//! is plaintext, or the server led the sign-in there from an address the engine accepts. The app
//! words each differently, so a connect must carry which.
//!
//! The server here is an HTTPS server on `127.0.0.1` whose certificate the account has accepted,
//! and it answers every request by sending it on to a plaintext address off this device.

use std::sync::Arc;

use mailcal_account::{
    AccountError, CertificateException, InsecureCause, connect_caldav,
    connect_carddav_contact_providers, load_str,
};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

/// Starts an HTTPS server for `127.0.0.1` that redirects every request to plaintext, and returns
/// its port and the certificate it serves.
async fn https_leading_to_plaintext() -> (u16, CertificateDer<'static>) {
    let key = rcgen::KeyPair::generate().expect("key pair");
    let params =
        rcgen::CertificateParams::new(vec!["127.0.0.1".to_owned()]).expect("certificate params");
    let der = params.self_signed(&key).expect("certificate").der().clone();
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
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match tls.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let _ = tls
                    .write_all(
                        b"HTTP/1.1 307 Temporary Redirect\r\n\
                          Location: http://dav.example.invalid/dav/\r\n\
                          Content-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                let _ = tls.shutdown().await;
            });
        }
    });
    (port, der)
}

fn cause(error: &AccountError) -> Option<InsecureCause> {
    match error {
        AccountError::InsecureServer { cause, .. } => Some(*cause),
        _ => None,
    }
}

/// An account at a secure address, which the server sends on to plaintext: the server led it
/// there, which only its operator, or a secure address for the same service, clears.
#[tokio::test]
async fn a_secure_address_the_server_leads_to_plaintext_is_one_the_server_led_there() {
    let (port, certificate) = https_leading_to_plaintext().await;
    let exception = CertificateException::new("127.0.0.1", &certificate);
    let account = load_str(&format!(
        "[caldav]\nbase_url = \"https://127.0.0.1:{port}/dav\"\nusername = \"alice\"\n\
         password = \"pw\"\n\n[[certificate_exception]]\nserver_name = \"{}\"\nsha256 = \"{}\"\n",
        exception.server_name, exception.sha256
    ))
    .expect("a calendar-and-contacts account loads");

    let calendar = connect_caldav(&account, None).await.err().expect("refused");
    let contacts = connect_carddav_contact_providers(&account, None)
        .await
        .err()
        .expect("refused");

    assert_eq!(
        cause(&calendar),
        Some(InsecureCause::LedToPlaintext),
        "{calendar}"
    );
    assert_eq!(
        cause(&contacts),
        Some(InsecureCause::LedToPlaintext),
        "{contacts}"
    );
}

/// An account whose own address is plaintext off this device: its https address clears it.
#[tokio::test]
async fn a_plaintext_address_off_this_device_is_the_accounts_own() {
    let account = load_str(
        "[caldav]\nbase_url = \"http://dav.example.invalid/dav\"\nusername = \"alice\"\n\
         password = \"pw\"\n",
    )
    .expect("a calendar-and-contacts account loads");

    let calendar = connect_caldav(&account, None).await.err().expect("refused");

    assert_eq!(
        cause(&calendar),
        Some(InsecureCause::PlaintextAddress),
        "{calendar}"
    );
}
