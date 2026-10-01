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

const DAV_ONLY: &str = r#"
[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice"
password = "pw"
"#;

/// A stored account with no mailbox boots as what it is: listed before its dial with nothing
/// known about its calendar, kept out of the mail surfaces, and named by its DAV login.
#[test]
fn a_stored_account_without_a_mailbox_boots_as_one() {
    let registry: crate::SharedRegistry = super::super::AccountRegistry::new();
    let store: std::sync::Arc<dyn crate::AccountCredentialStore> =
        std::sync::Arc::new(crate::tests::RecordingCredentialStore::default());
    let sink = crate::token_sink::token_sink(&registry, &store);
    let prepared = crate::boot::prepare_stored_account(
        DAV_ONLY,
        &sink,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("a calendar-only account prepares");

    assert_eq!(prepared.account.id.as_str(), "alice@dav:127.0.0.1:1");
    assert!(!prepared.account.uses_mail);
    assert!(!prepared.account.dialled);
    assert!(prepared.connected.imap().is_none(), "no mailbox to watch");
}

#[tokio::test]
async fn a_dial_of_an_account_without_a_mailbox_opens_its_calendar() {
    let config = mailcal_account::load_str(DAV_ONLY).unwrap();
    let failure = dial(&ConnectedAccount::imap_account(config, None)).await;
    assert!(failure.starts_with("caldav"), "{failure}");
}

fn microsoft(capabilities: &[&str], granted: &[&str]) -> ConnectedAccount {
    let names: Vec<String> = capabilities
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    let shape =
        mailcal_account::AccountShape::read(&format!("capabilities = [{}]", names.join(", ")))
            .unwrap();
    let config = mailcal_account::MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: Vec::new(),
        refresh_token: mailcal_account::Secret::new("refresh".to_owned()),
        granted_scopes: Some(granted.iter().map(|scope| (*scope).to_owned()).collect()),
        shape,
    };
    let id = config.account_id().unwrap();
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id,
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .unwrap();
    ConnectedAccount::Microsoft { config, tokens }
}

#[test]
fn a_use_the_grant_withholds_is_not_opened() {
    let entry = microsoft(
        &["mail", "contacts", "colleagues"],
        &["Mail.ReadWrite", "Contacts.ReadWrite"],
    );
    let opened = AccountDial::from_entry(&entry).capabilities().clone();
    assert!(opened.contains(mailcal_account::Capability::Contacts));
    assert!(!opened.contains(mailcal_account::Capability::Colleagues));
}

/// Mail unticked on the consent screen, calendar kept: the account opens as one without mail, so
/// its calendar works rather than the whole account failing on the mailbox it may not read.
#[test]
fn a_withheld_mailbox_leaves_an_account_without_mail() {
    let entry = microsoft(&["mail", "calendar"], &["User.Read", "Calendars.ReadWrite"]);
    let opened = AccountDial::from_entry(&entry).capabilities().clone();
    assert!(!opened.contains(mailcal_account::Capability::Mail));
    assert!(opened.contains(mailcal_account::Capability::Calendar));
    assert!(
        !entry
            .opened_capabilities()
            .contains(mailcal_account::Capability::Mail)
    );
}

/// A calendar the grant withholds is still a calendar the person chose, so it raises the prompt
/// to sign in again rather than staying silently empty; and nothing is dialled to learn it.
#[tokio::test]
async fn a_withheld_calendar_asks_for_consent_without_being_dialled() {
    let entry = microsoft(&["calendar"], &["offline_access", "User.Read"]);
    let id = engine_api::AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
    let outcome = AccountDial::from_entry(&entry)
        .run(&id, TimeZoneId::utc())
        .await
        .unwrap_or_else(|failure| panic!("nothing was dialled: {failure}"));
    assert!(outcome.calendar_reauth_required);
    assert!(outcome.account.calendar_providers.is_empty());
}
