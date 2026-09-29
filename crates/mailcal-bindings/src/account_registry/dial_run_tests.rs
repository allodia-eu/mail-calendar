//! A dial opens only what the account is used for.
//!
//! Every server here is a local port nothing listens on, so each attempt fails at once, and the
//! failure says which capability the dial tried.

use engine_api::TimeZoneId;

use crate::{ConnectedAccount, account_registry::dial::AccountDial};

const IMAP: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"

[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice@example.org"
password = "pw"
"#;

fn entry(capabilities: &str) -> ConnectedAccount {
    let config = mailcal_account::load_str(&format!("{capabilities}{IMAP}")).unwrap();
    ConnectedAccount::imap_account(config, None)
}

async fn dial(entry: &ConnectedAccount) -> String {
    let id = engine_api::AccountId::try_from("alice@example.org@localhost").unwrap();
    AccountDial::from_entry(entry)
        .run(&id, TimeZoneId::utc())
        .await
        .err()
        .expect("nothing listens here")
        .to_string()
}

#[tokio::test]
async fn an_account_used_for_mail_fails_on_its_mailbox() {
    let failure = dial(&entry("")).await;
    assert!(failure.starts_with("imap"), "{failure}");
}

#[tokio::test]
async fn an_account_not_used_for_mail_never_dials_its_mailbox() {
    // Used for its calendar alone: the failure is the calendar's, because the mailbox was
    // never tried.
    let failure = dial(&entry("capabilities = [\"calendar\"]\n")).await;
    assert!(failure.starts_with("caldav"), "{failure}");
}

#[test]
fn an_account_not_used_for_mail_has_no_mailbox_to_watch() {
    assert!(entry("").imap().is_some());
    assert!(
        entry("capabilities = [\"calendar\", \"contacts\"]\n")
            .imap()
            .is_none()
    );
}
