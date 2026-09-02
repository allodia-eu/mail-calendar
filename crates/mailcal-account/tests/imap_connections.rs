//! An IMAP account's folders and watches share its connections, so binding a folder opens no
//! socket of its own.
//!
//! The server here is a small scripted IMAP responder over TLS that counts the connections it
//! accepts, because the connection is what a server with a per-user limit counts. It answers every
//! command generically and reports a folder list with role folders, an untagged folder and a
//! nested one, which is all the dial needs.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use engine_core::{ids::AccountId, sync::SyncScope};
use engine_provider::Provider as _;
use mailcal_account::{
    AccountConfig, CertificateException, ImapConnections, connect_imap_mailbox,
    connect_imap_watcher, connect_mail_providers, load_str,
};
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

/// The folder list the server reports. Archive is tagged `\All`, as some servers do for a real
/// folder, Starred is a `\Flagged` view of mail filed elsewhere, and two folders carry no role.
const LIST: &str = "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n\
                    * LIST (\\HasNoChildren \\Sent) \"/\" \"Sent\"\r\n\
                    * LIST (\\HasNoChildren \\Drafts) \"/\" \"Drafts\"\r\n\
                    * LIST (\\HasNoChildren \\Trash) \"/\" \"Trash\"\r\n\
                    * LIST (\\HasNoChildren \\All) \"/\" \"Archive\"\r\n\
                    * LIST (\\HasNoChildren \\Flagged) \"/\" \"Starred\"\r\n\
                    * LIST (\\HasChildren) \"/\" \"Projects\"\r\n\
                    * LIST (\\HasNoChildren) \"/\" \"Projects/2026\"\r\n";

/// The reply to one tagged command line.
fn reply(line: &str) -> String {
    let mut words = line.split_whitespace();
    let tag = words.next().unwrap_or("*");
    let command = words.next().unwrap_or_default().to_ascii_uppercase();
    match command.as_str() {
        "CAPABILITY" => format!("* CAPABILITY IMAP4rev1 IDLE SPECIAL-USE\r\n{tag} OK done\r\n"),
        "LIST" => format!("{LIST}{tag} OK LIST done\r\n"),
        "EXAMINE" | "SELECT" => {
            format!("* 0 EXISTS\r\n* OK [UIDVALIDITY 1] ok\r\n{tag} OK [READ-ONLY] done\r\n")
        }
        _ => format!("{tag} OK done\r\n"),
    }
}

/// Starts the scripted server for `127.0.0.1`, returning its port, the certificate it serves and
/// the count of connections it has accepted.
async fn imap_server() -> (
    u16,
    rustls::pki_types::CertificateDer<'static>,
    Arc<AtomicUsize>,
) {
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
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&accepted);
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut tls = BufReader::new(tls);
                if tls.write_all(b"* OK ready\r\n").await.is_err() {
                    return;
                }
                let mut line = String::new();
                while matches!(tls.read_line(&mut line).await, Ok(read) if read > 0) {
                    // A watch's `DONE` and `IDLE` continuation are not tagged commands.
                    let answer = if line.trim().eq_ignore_ascii_case("DONE") {
                        String::new()
                    } else if line.to_ascii_uppercase().contains(" IDLE") {
                        "+ idling\r\n".to_owned()
                    } else {
                        reply(&line)
                    };
                    if tls.write_all(answer.as_bytes()).await.is_err() {
                        return;
                    }
                    line.clear();
                }
            });
        }
    });
    (port, der, accepted)
}

/// An account pointed at the server, trusting its certificate by exception.
fn account(port: u16, served: &rustls::pki_types::CertificateDer<'static>) -> AccountConfig {
    let exception = CertificateException::new("127.0.0.1", served);
    load_str(&format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"127.0.0.1\"\n\
         username = \"someone@example.com\"\npassword = \"secret\"\n\
         \n[[certificate_exception]]\nserver_name = \"{}\"\nsha256 = \"{}\"\n",
        exception.server_name, exception.sha256,
    ))
    .expect("a valid account config")
}

fn account_id() -> AccountId {
    AccountId::try_from("someone@127.0.0.1").expect("account id")
}

/// The mailbox each provider is bound to, in the order the dial returned them.
fn bound_mailboxes(providers: &[Box<dyn engine_provider::Provider>]) -> Vec<String> {
    providers
        .iter()
        .map(|provider| match provider.email_scope(&account_id()) {
            SyncScope::ImapMailbox { mailbox, .. } => mailbox.as_str().to_owned(),
            other => panic!("an IMAP folder provider bound to {other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_dial_binds_every_folder_the_account_lists() {
    // A folder the dial leaves out is in no account pass and is watched by nothing, so its mail
    // arrives only when someone opens it. Role or none, nested or not, every listed folder is
    // bound, and the Inbox once, first; a view of mail filed elsewhere (Starred) is not.
    let (port, served, _accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();

    let providers = connect_mail_providers(&connections, &config, None, &account_id())
        .await
        .expect("the dial");

    assert_eq!(
        bound_mailboxes(&providers),
        [
            "INBOX",
            "Sent",
            "Drafts",
            "Trash",
            "Archive",
            "Projects",
            "Projects/2026",
        ],
    );
}

#[tokio::test]
async fn a_dial_binds_every_folder_over_one_login() {
    let (port, served, accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();

    let providers = connect_mail_providers(&connections, &config, None, &account_id())
        .await
        .expect("the dial");
    let opened = connect_imap_mailbox(&connections, &config, None, "Projects")
        .await
        .expect("a folder opened on demand");

    assert_eq!(providers.len(), 7, "every listed folder");
    drop(opened);
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        1,
        "binding a folder opened a socket of its own",
    );
}

#[tokio::test]
async fn a_watch_takes_the_accounts_resting_connection_rather_than_dialling() {
    let (port, served, accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();
    connect_mail_providers(&connections, &config, None, &account_id())
        .await
        .expect("the dial");

    let _watch = connect_imap_watcher(&connections, &config, None, "INBOX")
        .await
        .expect("the server offers IDLE");

    assert_eq!(accepted.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn folders_opened_together_before_any_dial_connect_the_account_once() {
    let (port, served, accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();

    let opened = futures::future::join_all(
        ["INBOX", "Sent", "Projects"]
            .map(|mailbox| connect_imap_mailbox(&connections, &config, None, mailbox)),
    )
    .await;

    assert!(opened.iter().all(Result::is_ok));
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        1,
        "each caller that found no account connected one of its own",
    );
}

#[tokio::test]
async fn every_dial_logs_in_afresh() {
    // A dial's login is the one whose refusal can mean the password is wrong, so a dial never
    // rides an account a previous dial connected.
    let (port, served, accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();

    for _ in 0..2 {
        connect_mail_providers(&connections, &config, None, &account_id())
            .await
            .expect("the dial");
    }

    assert_eq!(accepted.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn invalidating_the_connections_makes_the_next_call_dial() {
    let (port, served, accepted) = imap_server().await;
    let config = account(port, &served);
    let connections = ImapConnections::new();
    let providers = connect_mail_providers(&connections, &config, None, &account_id())
        .await
        .expect("the dial");

    connections.invalidate();
    providers[0]
        .sync_mailboxes(&account_id(), None)
        .await
        .expect("the folder list, over a new connection");

    assert_eq!(accepted.load(Ordering::SeqCst), 2);
}
