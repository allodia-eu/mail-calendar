//! Setting up an account for a chosen subset of mail, calendar and contacts, and an address book
//! on a server of its own.

use super::{AccountSetup, SetupCredential, build_config_toml};
use crate::{Capabilities, Capability, ConfigError, ConnectionSecurity, load_str};

fn uses(capabilities: &[Capability]) -> Capabilities {
    capabilities.iter().copied().collect()
}

/// A calendar-and-contacts server with no mailbox, as a found card or the manual form hands it
/// over: the address as the login, and no mail server.
fn dav_only() -> AccountSetup {
    AccountSetup {
        imap_host: String::new(),
        username: "alice@cloud.example".to_owned(),
        credential: SetupCredential::Password("pw".to_owned()),
        smtp_host: None,
        caldav_base_url: Some("cloud.example/remote.php/dav".to_owned()),
        carddav_base_url: None,
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        accepted_certificate: None,
        uses: Some(uses(&[Capability::Calendar, Capability::Contacts])),
    }
}

fn mail_and_calendar() -> AccountSetup {
    AccountSetup {
        imap_host: "imap.example.net".to_owned(),
        username: "me@example.net".to_owned(),
        smtp_host: Some("smtp.example.net".to_owned()),
        caldav_base_url: Some("https://dav.example.net".to_owned()),
        uses: None,
        ..dav_only()
    }
}

#[test]
fn an_account_without_mail_stores_no_mail_server_and_needs_no_choice_stored() {
    let toml = build_config_toml(&dav_only()).unwrap();
    let config = load_str(&toml).unwrap();
    assert!(config.imap.is_none());
    assert!(config.smtp.is_none());
    let caldav = config.caldav.as_ref().unwrap();
    assert_eq!(caldav.base_url, "https://cloud.example/remote.php/dav");
    assert_eq!(caldav.username, "alice@cloud.example");
    assert_eq!(caldav.password.as_ref().unwrap().expose(), "pw");
    // A calendar endpoint already means calendar and contacts, so nothing is stored for it.
    assert!(!toml.contains("capabilities"));
    assert_eq!(
        config.capabilities(),
        [Capability::Calendar, Capability::Contacts]
            .into_iter()
            .collect()
    );
    assert_eq!(
        config.account_id().unwrap().as_str(),
        "alice@cloud.example@dav:cloud.example"
    );
}

#[test]
fn a_mail_server_typed_beside_a_choice_without_mail_is_left_out() {
    let mut setup = dav_only();
    setup.imap_host = "imap.cloud.example".to_owned();
    setup.smtp_host = Some("smtp.cloud.example".to_owned());
    let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
    assert!(config.imap.is_none());
    assert!(config.smtp.is_none());
}

#[test]
fn an_address_book_on_a_server_of_its_own_is_stored_with_the_login() {
    let mut setup = mail_and_calendar();
    setup.carddav_base_url = Some("contacts.example.net/dav".to_owned());
    let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
    let carddav = config.carddav.unwrap();
    assert_eq!(carddav.base_url, "https://contacts.example.net/dav");
    assert_eq!(carddav.username, "me@example.net");
    assert_eq!(carddav.password.as_ref().unwrap().expose(), "pw");
}

#[test]
fn an_address_book_alone_is_an_account_used_for_contacts() {
    let mut setup = dav_only();
    setup.caldav_base_url = None;
    setup.carddav_base_url = Some("https://cloud.example/dav".to_owned());
    setup.uses = Some(uses(&[Capability::Contacts]));
    let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
    assert_eq!(
        config.capabilities(),
        [Capability::Contacts].into_iter().collect()
    );
}

#[test]
fn a_choice_narrower_than_the_servers_is_stored() {
    let mut setup = mail_and_calendar();
    setup.uses = Some(uses(&[Capability::Mail, Capability::Calendar]));
    let toml = build_config_toml(&setup).unwrap();
    assert!(toml.contains("capabilities"));
    assert_eq!(
        load_str(&toml).unwrap().capabilities(),
        [Capability::Mail, Capability::Calendar]
            .into_iter()
            .collect()
    );
}

/// Contacts are looked for at the calendar's endpoint, so an account used for contacts but not
/// for its calendar keeps the calendar endpoint and stores the choice.
#[test]
fn contacts_without_the_calendar_keep_the_calendar_endpoint() {
    let mut setup = dav_only();
    setup.uses = Some(uses(&[Capability::Contacts]));
    let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
    assert!(config.caldav.is_some());
    assert_eq!(
        config.capabilities(),
        [Capability::Contacts].into_iter().collect()
    );
}

#[test]
fn a_chosen_use_without_its_server_is_refused() {
    let mut no_calendar = dav_only();
    no_calendar.caldav_base_url = None;
    no_calendar.uses = Some(uses(&[Capability::Calendar]));
    assert!(matches!(
        build_config_toml(&no_calendar),
        Err(ConfigError::Incomplete("calendar server"))
    ));

    let mut no_contacts = no_calendar.clone();
    no_contacts.uses = Some(uses(&[Capability::Contacts]));
    assert!(matches!(
        build_config_toml(&no_contacts),
        Err(ConfigError::Incomplete("address book server"))
    ));

    let mut no_mail = dav_only();
    no_mail.uses = Some(uses(&[Capability::Mail]));
    assert!(matches!(
        build_config_toml(&no_mail),
        Err(ConfigError::Incomplete("mail server"))
    ));
}

#[test]
fn a_choice_of_nothing_or_of_colleagues_is_refused() {
    let mut nothing = dav_only();
    nothing.uses = Some(Capabilities::default());
    assert!(matches!(
        build_config_toml(&nothing),
        Err(ConfigError::Incomplete(_))
    ));

    let mut colleagues = dav_only();
    colleagues.uses = Some(uses(&[Capability::Calendar, Capability::Colleagues]));
    assert!(matches!(
        build_config_toml(&colleagues),
        Err(ConfigError::Refused(_))
    ));
}
