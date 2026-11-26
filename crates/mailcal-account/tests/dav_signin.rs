//! An account without mail learns whether its sign-in still works from its calendar and its
//! contacts (`docs/accounts.md` rule 7), so their connects must tell a refused credential from
//! a server that could not help.
//!
//! The server here answers every request with one status and speaks no WebDAV: the first
//! request of a connect is what is under test.

use mailcal_account::{AccountConfig, AccountError, InsecureCause, connect_caldav, load_str};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Starts an HTTP server on `127.0.0.1` that answers every request with `status`, and returns
/// its port.
async fn server_answering(status: &'static str) -> u16 {
    server_answering_with(status, "").await
}

/// [`server_answering`], with `headers` (each line ending in `\r\n`) in every answer.
async fn server_answering_with(status: &'static str, headers: &'static str) -> u16 {
    server_responding(move |_| format!("{status}\r\n{headers}")).await
}

/// Starts an HTTP server on `127.0.0.1` that answers each request with the status line and
/// headers `respond` makes of the request's head, and returns its port.
async fn server_responding(respond: impl Fn(&str) -> String + Copy + Send + 'static) -> u16 {
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
                let head = respond(&String::from_utf8_lossy(&request));
                let response =
                    format!("HTTP/1.1 {head}Content-Length: 0\r\nConnection: close\r\n\r\n");
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

#[tokio::test]
async fn a_server_asking_only_for_another_method_is_neither_gone_nor_a_refused_signin() {
    // A `401` offering only Digest: the password was never sent, so it was not refused, and the
    // server answered, so looking it up again would find it where it is.
    let account = dav_only(
        server_answering_with(
            "401 Unauthorized",
            "WWW-Authenticate: Digest realm=\"dav\", nonce=\"n\"\r\n",
        )
        .await,
    );
    let err = connect_caldav(&account, None).await.err().expect("refused");
    assert!(!matches!(err, AccountError::SigninRejected(_)), "{err}");
    assert!(!err.is_gone(), "{err}");
}

/// HTTP Basic cannot carry a username holding `:`, so the engine sends no credential at all; it
/// never goes out as stored, so the person is asked to sign in again, as after a refused password.
#[tokio::test]
async fn a_stored_login_basic_cannot_carry_is_a_rejected_signin() {
    // A request that carries a credential is answered as an outage, so a credential that went
    // out cannot read as refused.
    let port = server_responding(|request| {
        if request.to_ascii_lowercase().contains("\r\nauthorization:") {
            "503 Service Unavailable\r\n".to_owned()
        } else {
            "401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\"\r\n".to_owned()
        }
    })
    .await;
    let account = load_str(&format!(
        "[caldav]\nbase_url = \"http://127.0.0.1:{port}/dav\"\nusername = \"al:ice\"\npassword = \"pw\"\n"
    ))
    .expect("a calendar-and-contacts account loads");

    let calendar = connect_caldav(&account, None).await.err().expect("refused");
    let contacts = mailcal_account::connect_carddav_contact_providers(&account, None)
        .await
        .err()
        .expect("refused");

    assert!(
        matches!(calendar, AccountError::SigninRejected(_)),
        "{calendar}"
    );
    assert!(
        matches!(contacts, AccountError::SigninRejected(_)),
        "{contacts}"
    );
}

/// The engine sends no sign-in over plaintext to a server off this device, and no new password
/// changes that, so the connect says the server is not reached securely rather than that the
/// sign-in was refused or the server could not be reached.
#[tokio::test]
async fn a_plaintext_server_off_this_device_is_an_insecure_server() {
    let account = load_str(
        "[caldav]\nbase_url = \"http://dav.example.invalid/dav\"\nusername = \"alice\"\npassword = \"pw\"\n",
    )
    .expect("a calendar-and-contacts account loads");

    let calendar = connect_caldav(&account, None).await.err().expect("refused");
    let contacts = mailcal_account::connect_carddav_contact_providers(&account, None)
        .await
        .err()
        .expect("refused");

    assert!(
        matches!(
            calendar,
            AccountError::InsecureServer {
                cause: InsecureCause::PlaintextAddress,
                ..
            }
        ),
        "{calendar}"
    );
    assert!(
        matches!(
            contacts,
            AccountError::InsecureServer {
                cause: InsecureCause::PlaintextAddress,
                ..
            }
        ),
        "{contacts}"
    );
    assert!(!calendar.is_gone(), "nothing to look up again");
}
