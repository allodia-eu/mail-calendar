use crate::{
    AccountSetup, Capabilities, Capability, ConfigError, ConnectionSecurity, NextcloudLogin,
    SetupCredential, build_config_toml, load_str,
};

fn login() -> NextcloudLogin {
    NextcloudLogin {
        server: "https://cloud.example.org".to_owned(),
        user: Some("alice".to_owned()),
        address: Some("alice@uni.example".to_owned()),
    }
}

/// A calendar and contacts account a Nextcloud sign-in made, signed in as the opaque id single
/// sign-on hands out.
fn setup(uses: &[Capability]) -> AccountSetup {
    AccountSetup {
        imap_host: String::new(),
        username: "f3a9c1".to_owned(),
        credential: SetupCredential::Nextcloud {
            app_password: "app-password".to_owned(),
            login: login(),
        },
        smtp_host: None,
        caldav_base_url: Some("https://cloud.example.org/remote.php/dav/".to_owned()),
        carddav_base_url: Some("https://cloud.example.org/remote.php/dav/".to_owned()),
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        accepted_certificate: None,
        uses: Some(uses.iter().copied().collect::<Capabilities>()),
    }
}

#[test]
fn the_app_password_is_stored_where_every_dav_password_is() {
    let toml = build_config_toml(&setup(&[Capability::Calendar, Capability::Contacts])).unwrap();
    let config = load_str(&toml).unwrap();
    let caldav = config.caldav.as_ref().unwrap();
    assert_eq!(caldav.username, "f3a9c1");
    assert_eq!(caldav.password.as_ref().unwrap().expose(), "app-password");
    let carddav = config.carddav.as_ref().unwrap();
    assert_eq!(carddav.username, "f3a9c1");
    assert_eq!(carddav.password.as_ref().unwrap().expose(), "app-password");
    assert_eq!(config.nextcloud, Some(login()));
    assert!(config.oauth.is_none());
}

#[test]
fn the_account_is_named_by_its_profile_address_and_identified_by_its_login() {
    let toml = build_config_toml(&setup(&[Capability::Calendar])).unwrap();
    let config = load_str(&toml).unwrap();
    assert_eq!(config.address(), "alice@uni.example");
    assert_eq!(config.username(), "f3a9c1");
    assert_eq!(
        config.account_id().unwrap().as_str(),
        "f3a9c1@dav:cloud.example.org"
    );
}

#[test]
fn a_login_without_a_profile_address_names_the_account_by_its_login() {
    let mut setup = setup(&[Capability::Calendar]);
    setup.credential = SetupCredential::Nextcloud {
        app_password: "app-password".to_owned(),
        login: NextcloudLogin {
            address: None,
            ..login()
        },
    };
    let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
    assert_eq!(config.address(), "f3a9c1");
}

#[test]
fn the_stored_table_survives_a_rewrite() {
    let config = load_str(&build_config_toml(&setup(&[Capability::Calendar])).unwrap()).unwrap();
    let rewritten = config.to_toml().unwrap();
    assert!(rewritten.contains("[nextcloud]"), "{rewritten}");
    assert_eq!(load_str(&rewritten).unwrap().nextcloud, Some(login()));
    let bare = NextcloudLogin {
        server: "https://cloud.example.org".to_owned(),
        user: None,
        address: None,
    };
    let table = bare.to_table();
    assert_eq!(table.len(), 1, "only the keys that are set: {table:?}");
}

#[test]
fn an_account_no_nextcloud_sign_in_made_has_no_table() {
    let mut password = setup(&[Capability::Calendar]);
    password.credential = SetupCredential::Password("typed".to_owned());
    let toml = build_config_toml(&password).unwrap();
    assert!(!toml.contains("nextcloud"), "{toml}");
    let config = load_str(&toml).unwrap();
    assert_eq!(config.nextcloud, None);
    assert!(!config.to_toml().unwrap().contains("nextcloud"));
    assert!(config.nextcloud_app_password().is_none());
}

#[test]
fn a_nextcloud_sign_in_never_makes_a_mailbox() {
    let mut mail = setup(&[Capability::Mail, Capability::Calendar]);
    mail.imap_host = "imap.example.org".to_owned();
    assert!(matches!(
        build_config_toml(&mail),
        Err(ConfigError::Refused(_))
    ));
}

#[test]
fn an_empty_app_password_is_incomplete() {
    let mut empty = setup(&[Capability::Calendar]);
    empty.credential = SetupCredential::Nextcloud {
        app_password: String::new(),
        login: login(),
    };
    assert!(matches!(
        build_config_toml(&empty),
        Err(ConfigError::Incomplete("password"))
    ));
}

#[test]
fn signing_in_again_replaces_the_login_and_the_password_together() {
    let config = load_str(
        &build_config_toml(&setup(&[Capability::Calendar, Capability::Contacts])).unwrap(),
    )
    .unwrap();
    let again = config.with_nextcloud_password(
        "alice@uni.example",
        "new-password",
        "https://cloud.example.org/remote.php/dav/",
        NextcloudLogin {
            address: Some("alice@uni.example".to_owned()),
            ..login()
        },
    );
    for endpoint in [
        again.caldav.as_ref().unwrap().username.as_str(),
        again.carddav.as_ref().unwrap().username.as_str(),
    ] {
        assert_eq!(endpoint, "alice@uni.example");
    }
    let (stored, name, password) = again.nextcloud_app_password().unwrap();
    assert_eq!(stored.user.as_deref(), Some("alice"));
    assert_eq!(name, "alice@uni.example");
    assert_eq!(password.expose(), "new-password");
}

#[test]
fn a_typed_password_is_not_a_nextcloud_one() {
    let config = load_str(&build_config_toml(&setup(&[Capability::Calendar])).unwrap()).unwrap();
    let typed = config.with_password("typed");
    assert_eq!(typed.nextcloud, None);
    assert_eq!(typed.address(), "f3a9c1");
}

#[test]
fn signing_in_moves_both_uses_to_the_server_s_own_dav_endpoint() {
    let mut typed = setup(&[Capability::Calendar, Capability::Contacts]);
    typed.credential = SetupCredential::Password("typed".to_owned());
    typed.caldav_base_url = Some("https://cloud.example.org/.well-known/caldav".to_owned());
    typed.carddav_base_url = Some("https://cloud.example.org/.well-known/carddav".to_owned());
    let config = load_str(&build_config_toml(&typed).unwrap()).unwrap();
    let signed = config.with_nextcloud_password(
        "f3a9c1",
        "app-password",
        "https://cloud.example.org/remote.php/dav/",
        login(),
    );
    for base_url in [
        &signed.caldav.as_ref().unwrap().base_url,
        &signed.carddav.as_ref().unwrap().base_url,
    ] {
        assert_eq!(base_url, "https://cloud.example.org/remote.php/dav/");
    }
}
