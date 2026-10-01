use crate::{Capability, ConfigError, ConnectionSecurity, load_str};

const STANDARDS: &str = r#"
[imap]
addr = "imap.example.org:993"
server_name = "imap.example.org"
username = "alice@example.org"
password = "old"

[smtp]
addr = "smtp.example.org:465"
server_name = "smtp.example.org"

[caldav]
base_url = "https://dav.example.org/dav"
username = "alice@example.org"
password = "old"
calendar = "/dav/calendars/alice/work/"
"#;

#[test]
fn unchanged_settings_move_nothing_and_pin_the_id_and_uses() {
    let config = load_str(STANDARDS).unwrap();
    let edited = config.with_endpoints(&config.endpoints()).unwrap();

    assert!(edited.moved.iter().next().is_none());
    assert_eq!(
        edited
            .config
            .shape
            .id
            .as_ref()
            .map(engine_core::ids::AccountId::as_str),
        Some("alice@example.org@imap.example.org")
    );
    assert_eq!(
        edited.config.shape.capabilities,
        Some(config.capabilities())
    );
    assert_eq!(
        edited.config.caldav.as_ref().unwrap().calendar.as_deref(),
        Some("/dav/calendars/alice/work/"),
        "the calendar chosen on the same server is kept"
    );
    let stored = edited.config.to_toml().unwrap();
    assert!(stored.contains("password = \"old\""), "{stored}");
}

#[test]
fn a_new_port_security_or_password_keeps_the_data_and_a_new_host_does_not() {
    let config = load_str(STANDARDS).unwrap();
    let mut edit = config.endpoints();
    edit.imap_host = Some("imap.example.org:143".to_owned());
    edit.imap_security = ConnectionSecurity::StartTls;
    edit.password = Some("new".to_owned());
    let kept = config.with_endpoints(&edit).unwrap();
    assert!(kept.moved.iter().next().is_none());
    let imap = kept.config.imap.as_ref().unwrap();
    assert_eq!(imap.addr, "imap.example.org:143");
    assert_eq!(imap.security, ConnectionSecurity::StartTls);
    assert_eq!(
        kept.config
            .caldav
            .as_ref()
            .unwrap()
            .password
            .as_ref()
            .unwrap()
            .expose(),
        "new",
        "one sign-in for every endpoint"
    );

    let mut moved = config.endpoints();
    moved.imap_host = Some("mail.example.net".to_owned());
    moved.caldav_url = Some("cloud.example.net/remote.php/dav".to_owned());
    let edited = config.with_endpoints(&moved).unwrap();
    let domains: Vec<Capability> = edited.moved.iter().collect();
    assert_eq!(
        domains,
        [Capability::Mail, Capability::Calendar, Capability::Contacts]
    );
    assert_eq!(
        edited.config.imap.as_ref().unwrap().addr,
        "mail.example.net:993"
    );
    let caldav = edited.config.caldav.as_ref().unwrap();
    assert_eq!(caldav.base_url, "https://cloud.example.net/remote.php/dav");
    assert!(
        caldav.calendar.is_none(),
        "a calendar on another server is chosen again"
    );
    assert_eq!(
        edited
            .config
            .shape
            .id
            .as_ref()
            .map(engine_core::ids::AccountId::as_str),
        Some("alice@example.org@imap.example.org"),
        "the id survives a new host"
    );
}

#[test]
fn a_use_cannot_lose_its_server_and_an_oauth_account_takes_no_password() {
    let config = load_str(STANDARDS).unwrap();
    let mut edit = config.endpoints();
    edit.caldav_url = None;
    assert!(matches!(
        config.with_endpoints(&edit),
        Err(ConfigError::Refused(_))
    ));

    let mut blank = config.endpoints();
    blank.username = " ".to_owned();
    assert!(matches!(
        config.with_endpoints(&blank),
        Err(ConfigError::Incomplete(_))
    ));

    let oauth = load_str(OAUTH).unwrap();
    let mut typed = oauth.endpoints();
    typed.password = Some("typed".to_owned());
    assert!(matches!(
        oauth.with_endpoints(&typed),
        Err(ConfigError::Refused(_))
    ));
    let kept = oauth.with_endpoints(&oauth.endpoints()).unwrap();
    assert!(kept.config.is_oauth());
    assert!(kept.config.imap.as_ref().unwrap().password.is_none());
}

const OAUTH: &str = r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"

[oauth]
client_id = "client-abc"
refresh_token = "rt-value"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access"]
issuer = "https://auth.example.net"
"#;

#[test]
fn a_server_reached_by_address_keeps_the_name_it_is_verified_by() {
    let config = load_str(
        "[imap]\naddr = \"127.0.0.1:993\"\nserver_name = \"mail.example.org\"\n\
         username = \"alice\"\npassword = \"pw\"\n",
    )
    .unwrap();
    let mut edit = config.endpoints();
    edit.imap_host = Some("127.0.0.1:143".to_owned());
    let edited = config.with_endpoints(&edit).unwrap();
    assert_eq!(
        edited.config.imap.as_ref().unwrap().server_name,
        "mail.example.org"
    );
    assert!(edited.moved.iter().next().is_none());
}
