//! Which servers a fresh lookup moves an account onto, and how often it is asked.

use std::time::{Duration, Instant};

use mailcal_autodetect::DetectedDav;

use super::{RETRY_AFTER, Rediscovery, moved_endpoints};
use crate::{AccountEndpoints, ConnectionSecurity};

const ADDRESS: &str = "alice@example.com";
const CALDAV: &str = "https://d100.caldav.example.com/.well-known/caldav";
const CARDDAV: &str = "https://d100.carddav.example.com/.well-known/carddav";

fn stored() -> AccountEndpoints {
    AccountEndpoints {
        imap_host: None,
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_host: None,
        smtp_security: ConnectionSecurity::ImplicitTls,
        caldav_url: Some(CALDAV.to_owned()),
        carddav_url: Some(CARDDAV.to_owned()),
        username: ADDRESS.to_owned(),
        password: None,
    }
}

fn found(caldav: &str, carddav: &str) -> DetectedDav {
    DetectedDav {
        caldav_url: Some(caldav.to_owned()),
        carddav_url: Some(carddav.to_owned()),
    }
}

fn moved(found: &DetectedDav) -> Option<AccountEndpoints> {
    moved_endpoints(&stored(), found, ADDRESS).0
}

fn asked(found: &DetectedDav) -> Option<mailcal_app::ServerChange> {
    moved_endpoints(&stored(), found, ADDRESS).1
}

#[test]
fn a_server_the_provider_moved_is_followed_and_the_other_kept() {
    let new_caldav = "https://d200.caldav.example.com/.well-known/caldav";
    let edit = moved(&found(new_caldav, CARDDAV)).expect("the calendar moved");
    assert_eq!(edit.caldav_url.as_deref(), Some(new_caldav));
    assert_eq!(edit.carddav_url.as_deref(), Some(CARDDAV));
    assert_eq!(edit.username, ADDRESS, "the login stays");
    assert_eq!(edit.password, None, "the stored password stays");
}

#[test]
fn the_same_servers_or_another_providers_move_nothing() {
    assert!(moved(&found(CALDAV, CARDDAV)).is_none(), "nothing moved");
    assert!(
        moved(&found("https://dav.elsewhere.example.net/", CARDDAV)).is_none(),
        "a stored password never follows DNS to another provider"
    );
    assert!(
        moved(&DetectedDav {
            caldav_url: None,
            carddav_url: None,
        })
        .is_none(),
        "finding nothing is no reason to drop what is stored"
    );
}

#[test]
fn an_account_is_looked_for_at_most_once_an_hour() {
    let rediscovery = Rediscovery::default();
    let start = Instant::now();
    assert!(rediscovery.due("a", start));
    assert!(!rediscovery.due("a", start + Duration::from_secs(60)));
    assert!(rediscovery.due("b", start), "each account on its own");
    assert!(rediscovery.due("a", start + RETRY_AFTER));
}

/// Boots an app over one calendar-only account whose server is `base_url`.
fn boot(name: &str, base_url: &str) -> (std::sync::Arc<crate::MailcalApp>, String) {
    use std::sync::{Arc, mpsc};

    use crate::{
        LogLevel, MailcalApp,
        tests::{
            ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle,
            temp_data_dir,
        },
    };
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![format!(
            "[caldav]\nbase_url = \"{base_url}\"\nusername = \"alice\"\npassword = \"pw\"\n"
        )],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots");
    app.set_dns_resolver(Box::new(NoRecords));
    let id = app.accounts_snapshot().accounts[0].id.clone();
    (app, id)
}

/// A server that answers every request `404 Not Found`: there, but without the start it was
/// given.
fn gone_server() -> String {
    answering(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// A server that answers every request with `response`.
fn answering(response: &'static [u8]) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response);
        }
    });
    format!("http://{addr}/dav")
}

/// Waits up to twenty seconds for `id` to be looked for again.
fn looked_for_again(app: &crate::MailcalApp, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !app.rediscovery.attempts.lock().unwrap().contains_key(id) {
        assert!(
            Instant::now() < deadline,
            "the account was never looked for again"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A calendar server that says its start is gone is looked for again once the reconnect is done
/// (RFC 6764 §6): the trigger, end to end.
#[test]
fn a_calendar_server_whose_start_is_gone_is_looked_for_again() {
    let (app, id) = boot("dav-rediscovery-gone", &gone_server());

    looked_for_again(&app, &id);
}

/// A calendar server that refuses the sign-in is looked for again too, also for an account it is
/// all of: a provider that moved can leave its old server refusing every credential.
#[test]
fn a_calendar_server_that_refuses_the_sign_in_is_looked_for_again() {
    let refusing = answering(
        b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\"\r\n\
          Content-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let (app, id) = boot("dav-rediscovery-refused", &refusing);

    looked_for_again(&app, &id);
}

/// A calendar server that does not answer is tried again as it is, and never looked for again: an
/// outage says nothing about where the server is.
#[test]
fn a_calendar_server_that_does_not_answer_is_not_looked_for_again() {
    let (app, id) = boot("dav-rediscovery-outage", "http://127.0.0.1:1/dav");

    let deadline = Instant::now() + Duration::from_secs(20);
    while !app.disconnected.lock().unwrap().contains(&id) {
        assert!(Instant::now() < deadline, "the launch dial never ended");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !app.rediscovery.attempts.lock().unwrap().contains_key(&id),
        "an outage sent the account to be looked for again"
    );
}

/// DNS that answers every lookup with nothing, so the trigger test reaches no network.
struct NoRecords;

impl crate::MxResolver for NoRecords {
    fn resolve_mx(&self, _domain: String) -> Result<crate::MxResolution, crate::DnsError> {
        Ok(crate::MxResolution {
            records: Vec::new(),
            authentic_data: false,
        })
    }

    fn resolve_srv(&self, _name: String) -> Result<crate::SrvResolution, crate::DnsError> {
        Ok(crate::SrvResolution {
            records: Vec::new(),
            authentic_data: false,
        })
    }
}

#[test]
fn a_server_on_another_site_is_asked_about_naming_both_sites() {
    let elsewhere = "https://dav.elsewhere.example.net/.well-known/caldav";
    let change = asked(&found(elsewhere, CARDDAV)).expect("asked about");
    assert_eq!(change.previous_site, "example.com");
    assert_eq!(change.proposed_site, "example.net");
    assert_eq!(change.calendar_url.as_deref(), Some(elsewhere));
    assert_eq!(change.contacts_url, None, "the contacts did not move");
}

#[test]
fn one_service_is_followed_while_the_other_is_asked_about() {
    let new_caldav = "https://d200.caldav.example.com/.well-known/caldav";
    let elsewhere = "https://books.elsewhere.example.net/.well-known/carddav";
    let (edit, change) = moved_endpoints(&stored(), &found(new_caldav, elsewhere), ADDRESS);
    let edit = edit.expect("the calendar moved within its site");
    assert_eq!(edit.caldav_url.as_deref(), Some(new_caldav));
    assert_eq!(
        edit.carddav_url.as_deref(),
        Some(CARDDAV),
        "not followed yet"
    );
    let change = change.expect("the contacts are asked about");
    assert_eq!(change.contacts_url.as_deref(), Some(elsewhere));
    assert_eq!(change.calendar_url, None);
}

#[test]
fn the_same_servers_ask_nothing() {
    assert!(asked(&found(CALDAV, CARDDAV)).is_none());
}

#[test]
fn a_question_names_every_site_its_allow_would_send_the_credential_to() {
    let calendar = "https://dav.elsewhere.example.net/.well-known/caldav";
    let books = "https://books.another.example.org/.well-known/carddav";
    let change = asked(&found(calendar, books)).expect("asked about");
    assert_eq!(change.proposed_site, "example.net");
    assert_eq!(change.calendar_url.as_deref(), Some(calendar));
    assert_eq!(
        change.contacts_url, None,
        "the contacts moved to a site the question does not name"
    );
}

mod staged {
    use super::{CALDAV, stored};
    use crate::dav_rediscovery::staged::staged_change;

    #[test]
    fn every_server_the_account_stores_moves_to_the_one_named() {
        let mut harness = stored();
        harness.caldav_url = Some("https://localhost:12443".to_owned());
        harness.carddav_url = Some("https://localhost:12443".to_owned());
        let change =
            staged_change(&harness, " http://127.0.0.1:28080 ").expect("a URL stages a change");
        assert_eq!(
            change.calendar_url.as_deref(),
            Some("http://127.0.0.1:28080")
        );
        assert_eq!(
            change.contacts_url.as_deref(),
            Some("http://127.0.0.1:28080")
        );
        assert_eq!(
            (change.previous_site.as_str(), change.proposed_site.as_str()),
            ("localhost", "127.0.0.1"),
            "a host the Public Suffix List cannot place is named as it is, as the loopback harness needs"
        );
    }

    #[test]
    fn a_use_the_account_has_no_server_for_does_not_move() {
        let mut calendar_only = stored();
        calendar_only.carddav_url = None;
        let change = staged_change(&calendar_only, "https://dav.elsewhere.example.net/")
            .expect("the calendar moves");
        assert_eq!(change.contacts_url, None);
        assert_eq!(
            (change.previous_site.as_str(), change.proposed_site.as_str()),
            ("example.com", "example.net"),
            "a site is its registrable domain, as a real lookup names it"
        );
    }

    #[test]
    fn a_value_that_names_no_server_stages_nothing() {
        assert_eq!(staged_change(&stored(), "calendar"), None);
        assert_eq!(staged_change(&stored(), ""), None);
        let mut none = stored();
        none.caldav_url = None;
        none.carddav_url = None;
        assert_eq!(
            staged_change(&none, CALDAV),
            None,
            "an account with no DAV server has nothing to move"
        );
    }
}
