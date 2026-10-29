//! A calendar connect starts at the principal the last one stored, falls back to the configured
//! server only when that principal is gone, and reports the principal it found (RFC 6764 §6). A
//! configured path that is gone falls back to the well-known, for calendars and contacts alike.
//!
//! The server here answers the requests of a CalDAV walk by request line and records each one,
//! so the assertions are on what the server saw.

use std::sync::{Arc, Mutex};

use mailcal_account::{
    AccountConfig, DavStarts, connect_caldav_learning, connect_carddav_learning, load_str,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const PRINCIPAL: &str = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/p/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/p/</d:href></d:current-user-principal><c:calendar-home-set><d:href>/home/</d:href></c:calendar-home-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;
const LISTING: &str = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/home/cal/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>Calendar</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#;

fn answer(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A CalDAV server whose principal is `/p/`, whose home is `/home/`, and on which `/gone/` no
/// longer exists. Returns its origin and the request lines it has seen.
async fn server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let origin = format!("http://{}", listener.local_addr().expect("addr"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0_u8; 8192];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let head = String::from_utf8_lossy(&request).to_string();
                let line = head.lines().next().unwrap_or_default().to_owned();
                log.lock().unwrap().push(line.clone());
                let response = match line.split_whitespace().take(2).collect::<Vec<_>>()[..] {
                    ["OPTIONS", _] => answer("200 OK", "DAV: 1, calendar-access\r\n", ""),
                    ["PROPFIND", "/.well-known/caldav" | "/p/"] => {
                        answer("207 Multi-Status", "", PRINCIPAL)
                    }
                    ["PROPFIND", "/home/"] => answer("207 Multi-Status", "", LISTING),
                    _ => answer("404 Not Found", "", ""),
                };
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (origin, seen)
}

fn account(origin: &str, calendar_start: Option<String>) -> AccountConfig {
    let mut config = load_str(&format!(
        "[caldav]\nbase_url = \"{origin}\"\nusername = \"alice\"\npassword = \"pw\"\n"
    ))
    .expect("a calendar account loads");
    config.dav_starts = DavStarts {
        calendar: calendar_start,
        contacts: None,
    };
    config
}

fn first_request(seen: &Mutex<Vec<String>>) -> String {
    seen.lock().unwrap().first().cloned().unwrap_or_default()
}

#[tokio::test]
async fn a_first_connect_learns_the_principal() {
    let (origin, seen) = server().await;
    let connected = connect_caldav_learning(&account(&origin, None), None)
        .await
        .expect("connects");
    assert_eq!(connected.principal, Some(format!("{origin}/p/")));
    assert!(first_request(&seen).starts_with("PROPFIND /.well-known/caldav "));
}

#[tokio::test]
async fn a_stored_principal_is_where_the_connect_starts() {
    let (origin, seen) = server().await;
    let connected = connect_caldav_learning(&account(&origin, Some(format!("{origin}/p/"))), None)
        .await
        .expect("connects");
    assert_eq!(connected.principal, Some(format!("{origin}/p/")));
    assert!(first_request(&seen).starts_with("PROPFIND /p/ "));
    assert!(
        !seen
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains("well-known")),
        "the configured server was asked again: {:?}",
        seen.lock().unwrap()
    );
}

#[tokio::test]
async fn a_principal_that_is_gone_falls_back_to_the_configured_server() {
    let (origin, seen) = server().await;
    let connected =
        connect_caldav_learning(&account(&origin, Some(format!("{origin}/gone/"))), None)
            .await
            .expect("connects from the configured server");
    assert_eq!(connected.principal, Some(format!("{origin}/p/")));
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("PROPFIND /gone/ "), "{seen:?}");
    assert!(
        seen.iter()
            .any(|line| line.starts_with("PROPFIND /.well-known/caldav "))
    );
}

#[tokio::test]
async fn a_configured_path_is_where_the_connect_starts() {
    let (origin, seen) = server().await;
    let connected = connect_caldav_learning(&account(&format!("{origin}/p/"), None), None)
        .await
        .expect("connects");
    assert_eq!(connected.principal, Some(format!("{origin}/p/")));
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("PROPFIND /p/ "), "{seen:?}");
    assert!(
        !seen.iter().any(|line| line.contains("well-known")),
        "{seen:?}"
    );
}

/// A path stored before the engine asked it: it may never have answered, and the server's
/// well-known still finds the calendar.
#[tokio::test]
async fn a_configured_path_that_is_gone_falls_back_to_the_well_known() {
    let (origin, seen) = server().await;
    let connected = connect_caldav_learning(&account(&format!("{origin}/gone/"), None), None)
        .await
        .expect("connects from the well-known");
    assert_eq!(connected.principal, Some(format!("{origin}/p/")));
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("PROPFIND /gone/ "), "{seen:?}");
    assert!(
        seen.iter()
            .any(|line| line.starts_with("PROPFIND /.well-known/caldav ")),
        "{seen:?}"
    );
}

/// Contacts with no server of their own use the calendar's origin, and look for the address book
/// where a CardDAV server keeps it, not at the calendar's path.
#[tokio::test]
async fn contacts_on_the_calendar_server_start_at_its_carddav_well_known() {
    let (origin, seen) = server().await;
    let _ = connect_carddav_learning(&account(&format!("{origin}/p/"), None), None).await;
    assert!(
        first_request(&seen).starts_with("PROPFIND /.well-known/carddav "),
        "{:?}",
        seen.lock().unwrap()
    );
}

#[tokio::test]
async fn an_address_book_path_that_is_gone_falls_back_to_the_well_known() {
    let (origin, seen) = server().await;
    let config = load_str(&format!(
        "[carddav]\nbase_url = \"{origin}/gone/\"\nusername = \"alice\"\npassword = \"pw\"\n"
    ))
    .expect("a contacts account loads");
    let _ = connect_carddav_learning(&config, None).await;
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("PROPFIND /gone/ "), "{seen:?}");
    assert!(
        seen.iter()
            .any(|line| line.starts_with("PROPFIND /.well-known/carddav ")),
        "{seen:?}"
    );
}

#[tokio::test]
async fn a_principal_that_cannot_be_reached_changes_nothing() {
    let (origin, seen) = server().await;
    // Nothing listens on port 1: the stored principal cannot be reached, which says nothing
    // about whether it still exists.
    let err = connect_caldav_learning(
        &account(&origin, Some("http://127.0.0.1:1/p/".to_owned())),
        None,
    )
    .await
    .expect_err("unreachable");
    assert!(!err.is_gone(), "{err}");
    assert!(
        seen.lock().unwrap().is_empty(),
        "fell back to the configured server"
    );
}
