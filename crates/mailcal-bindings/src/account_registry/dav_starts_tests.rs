use mailcal_account::{DavStarts, DavStartsFile};

use crate::{ConnectedAccount, account_registry::AccountRegistry};

const CONFIG: &str = r#"
[caldav]
base_url = "https://dav.example.net"
username = "alice"
password = "pw"
"#;

fn entry() -> ConnectedAccount {
    ConnectedAccount::imap_account(mailcal_account::load_str(CONFIG).unwrap(), None)
}

fn file(name: &str) -> DavStartsFile {
    let dir = crate::tests::temp_data_dir(name);
    DavStartsFile::in_dir(dir)
}

fn principal(url: &str) -> DavStarts {
    DavStarts {
        calendar: Some(url.to_owned()),
        contacts: None,
    }
}

/// What a dial learned when its calendar connected and its server named `url`.
fn learned(url: &str) -> mailcal_account::DavLearned {
    mailcal_account::DavLearned {
        calendar: mailcal_account::Learned::Principal(url.to_owned()),
        contacts: mailcal_account::Learned::Nothing,
    }
}

fn stored(registry: &AccountRegistry, id: &str) -> DavStarts {
    match registry.dial(id).expect("registered") {
        crate::account_registry::AccountDial::Imap { config, .. } => config.dav_starts,
        _ => panic!("a standards account"),
    }
}

#[test]
fn a_learned_principal_is_dialled_from_and_survives_a_relaunch() {
    let file = file("dav-starts-learn");
    let registry = AccountRegistry::new();
    registry.keep_dav_starts_in(file.clone());
    registry.pre_register("alice".to_owned(), entry()).commit();

    registry.learn_dav_starts("alice", &learned("https://dav.example.net/alice/"));
    assert_eq!(
        stored(&registry, "alice"),
        principal("https://dav.example.net/alice/")
    );

    // The next launch registers the same account from its stored config.
    let relaunched = AccountRegistry::new();
    relaunched.keep_dav_starts_in(file);
    relaunched
        .pre_register("alice".to_owned(), entry())
        .commit();
    assert_eq!(
        stored(&relaunched, "alice"),
        principal("https://dav.example.net/alice/")
    );
}

#[test]
fn removing_the_account_forgets_its_principals() {
    let file = file("dav-starts-remove");
    let registry = AccountRegistry::new();
    registry.keep_dav_starts_in(file.clone());
    registry.pre_register("alice".to_owned(), entry()).commit();
    registry.learn_dav_starts("alice", &learned("https://dav.example.net/alice/"));

    registry.remove("alice");

    assert!(file.load().is_empty());
}

#[test]
fn a_dial_that_learned_nothing_keeps_what_was_stored() {
    let file = file("dav-starts-nothing");
    let registry = AccountRegistry::new();
    registry.keep_dav_starts_in(file);
    registry.pre_register("alice".to_owned(), entry()).commit();
    registry.learn_dav_starts("alice", &learned("https://dav.example.net/alice/"));

    registry.learn_dav_starts("alice", &mailcal_account::DavLearned::default());

    assert_eq!(
        stored(&registry, "alice"),
        principal("https://dav.example.net/alice/")
    );
}
