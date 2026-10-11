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
    moved_endpoints(&stored(), found, ADDRESS, "acct:0000")
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

/// A calendar server that does not answer at launch is looked for again once the reconnect is
/// done: the trigger, end to end, against a port nothing listens on.
#[test]
fn a_calendar_server_that_does_not_answer_is_looked_for_again() {
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
        vec![
            "[caldav]\nbase_url = \"http://127.0.0.1:1/dav\"\nusername = \"alice\"\npassword = \"pw\"\n"
                .to_owned(),
        ],
        temp_data_dir("dav-rediscovery-trigger").to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(RecordingCredentialStore::default()))),
    )
    .expect("the app boots");
    app.set_dns_resolver(Box::new(NoRecords));
    let id = app.accounts_snapshot().accounts[0].id.clone();

    let deadline = Instant::now() + Duration::from_secs(20);
    while !app.rediscovery.attempts.lock().unwrap().contains_key(&id) {
        assert!(
            Instant::now() < deadline,
            "the account was never looked for again"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
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
