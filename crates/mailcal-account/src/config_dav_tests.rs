//! A standards account that has no mailbox.

use crate::{Capability, ConfigError, load_str};

const DAV_ONLY: &str = r#"
[caldav]
base_url = "https://Cloud.Example/remote.php/dav"
username = "Alice"
password = "app-password"
"#;

const CARDDAV: &str = r#"
[carddav]
base_url = "https://contacts.example/dav"
username = "alice"
password = "other-password"
"#;

#[test]
fn an_account_with_a_calendar_and_no_mailbox_loads() {
    let config = load_str(DAV_ONLY).unwrap();
    assert!(config.imap.is_none());
    assert_eq!(config.username(), "Alice");
}

#[test]
fn an_account_without_a_mailbox_is_named_by_its_dav_login_and_host() {
    // Tagged so it can never meet the IMAP account `alice@cloud.example` for the same login.
    let config = load_str(DAV_ONLY).unwrap();
    assert_eq!(
        config.account_id().unwrap().as_str(),
        "alice@dav:cloud.example"
    );
}

#[test]
fn an_account_without_a_mailbox_is_used_for_what_its_endpoints_serve() {
    let listed = |text: &str| {
        load_str(text)
            .unwrap()
            .capabilities()
            .iter()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        listed(DAV_ONLY),
        [Capability::Calendar, Capability::Contacts]
    );
    // Contacts alone: an address book with nothing beside it.
    assert_eq!(listed(CARDDAV), [Capability::Contacts]);
}

#[test]
fn contacts_use_their_own_endpoint_when_the_account_has_one() {
    let config = load_str(&format!("{DAV_ONLY}{CARDDAV}")).unwrap();
    let contacts = config.carddav_endpoint().unwrap();
    assert_eq!(contacts.base_url, "https://contacts.example/dav");
    assert_eq!(contacts.password.unwrap().expose(), "other-password");
    // Without one, they are found from the calendar's.
    let calendar_only = load_str(DAV_ONLY).unwrap();
    assert_eq!(
        calendar_only.carddav_endpoint().unwrap().base_url,
        "https://Cloud.Example/remote.php/dav"
    );
}

#[test]
fn an_account_without_a_mailbox_round_trips() {
    let config = load_str(&format!("{DAV_ONLY}{CARDDAV}")).unwrap();
    let stored = config.to_toml().unwrap();
    assert!(!stored.contains("[imap]"), "{stored}");
    let reread = load_str(&stored).unwrap();
    assert_eq!(reread.to_toml().unwrap(), stored);
    assert_eq!(reread.account_id().unwrap(), config.account_id().unwrap());
}

#[test]
fn a_new_password_reaches_every_endpoint_that_stores_one() {
    let config = load_str(&format!("{DAV_ONLY}{CARDDAV}"))
        .unwrap()
        .with_password("new");
    assert_eq!(config.caldav.unwrap().password.unwrap().expose(), "new");
    assert_eq!(config.carddav.unwrap().password.unwrap().expose(), "new");
}

#[test]
fn an_account_without_a_mailbox_has_nothing_to_dial_for_mail() {
    let config = load_str(DAV_ONLY).unwrap();
    assert!(config.imap_password_credentials().is_none());
}

#[test]
fn a_config_that_names_no_server_is_refused() {
    let err = load_str("capabilities = [\"mail\"]\n").unwrap_err();
    assert!(matches!(err, ConfigError::NoEndpoint), "{err}");
}

const OAUTH_WITH_DAV: &str = r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"

[caldav]
base_url = "https://dav.example.net"
username = "you@example.net"

[oauth]
client_id = "client-abc"
refresh_token = "initial-refresh"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "imap", "smtp"]
"#;

#[tokio::test]
async fn an_oauth_account_hands_dav_its_token_with_the_login_it_belongs_to() {
    // A DAV server beside an OAuth mail server may take the access token only as a Basic
    // password for the account (soverin.net's does), which the transport learns from its
    // challenge. A bare bearer token names no login to pair it with, so the endpoint's own is
    // handed over beside it.
    use provider_caldav::Credentials;

    use crate::graph::test_support::{ratcheting_token_endpoint, source_at};

    let config = load_str(OAUTH_WITH_DAV).unwrap();
    let (token_endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(token_endpoint, None);
    let credentials = crate::config_dav::dav_credentials(
        &config,
        config.caldav_endpoint().unwrap(),
        Some(&tokens),
    )
    .await
    .unwrap();
    match credentials {
        Credentials::Token { username, token } => {
            assert_eq!(username, "you@example.net");
            assert_eq!(token, "AT-1");
        }
        other => panic!("expected the token beside its login, got {other:?}"),
    }
}

#[tokio::test]
async fn an_oauth_account_hands_contacts_their_own_login() {
    // An address book on its own endpoint may be a different login than the calendar's, and the
    // Basic form of the token is only accepted for the login it is paired with.
    use provider_caldav::Credentials;

    use crate::graph::test_support::{ratcheting_token_endpoint, source_at};

    let config = load_str(&format!(
        "{OAUTH_WITH_DAV}\n[carddav]\nbase_url = \"https://contacts.example.net\"\nusername = \"contacts-login\"\n"
    ))
    .unwrap();
    let (token_endpoint, _) = ratcheting_token_endpoint("initial-refresh");
    let tokens = source_at(token_endpoint, None);
    let credentials = crate::config_dav::dav_credentials(
        &config,
        config.carddav_endpoint().unwrap(),
        Some(&tokens),
    )
    .await
    .unwrap();
    match credentials {
        Credentials::Token { username, .. } => assert_eq!(username, "contacts-login"),
        other => panic!("expected the token beside its login, got {other:?}"),
    }
}
