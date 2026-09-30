//! An account without mail learns whether its sign-in still works from its calendar and its
//! contacts (`docs/accounts.md` rule 7), so their connects must tell a refused credential from
//! a server that could not help.
//!
//! The server here answers every request with one status and speaks no WebDAV: the first
//! request of a connect is what is under test.

use mailcal_account::{AccountConfig, AccountError, connect_caldav, load_str};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Starts an HTTP server on `127.0.0.1` that answers every request with `status`, and returns
/// its port.
async fn server_answering(status: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local addr").port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    port
}

/// A calendar-and-contacts account on the server at `port`, with no mailbox.
fn dav_only(port: u16) -> AccountConfig {
    load_str(&format!(
        "[caldav]\nbase_url = \"http://127.0.0.1:{port}/dav\"\nusername = \"alice\"\npassword = \"pw\"\n"
    ))
    .expect("a calendar-and-contacts account loads")
}

#[tokio::test]
async fn a_refused_password_on_the_calendar_is_a_rejected_signin() {
    let account = dav_only(server_answering("401 Unauthorized").await);
    let err = connect_caldav(&account, None).await.err().expect("refused");
    assert!(matches!(err, AccountError::SigninRejected(_)), "{err}");
}

#[tokio::test]
async fn a_refused_password_on_the_contacts_is_a_rejected_signin() {
    let account = dav_only(server_answering("401 Unauthorized").await);
    let err = mailcal_account::connect_carddav_contact_providers(&account, None)
        .await
        .err()
        .expect("refused");
    assert!(matches!(err, AccountError::SigninRejected(_)), "{err}");
}

#[tokio::test]
async fn a_calendar_the_server_will_not_serve_is_not_a_rejected_signin() {
    let account = dav_only(server_answering("403 Forbidden").await);
    let err = connect_caldav(&account, None).await.err().expect("refused");
    assert!(matches!(err, AccountError::CalDav(_)), "{err}");
}
