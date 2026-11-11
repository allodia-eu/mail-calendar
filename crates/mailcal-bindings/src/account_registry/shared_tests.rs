//! A shared mailbox has no entry of its own: it is dialled, watched and removed through the
//! account whose sign-in opens it.

use engine_api::SharedAccountId;
use mailcal_account::{JmapAccountConfig, Secret, SharedStoreChoice, SharedUse};

use super::{AccountDial, AccountRegistry};
use crate::ConnectedAccount;

fn jmap(email: &str) -> (String, ConnectedAccount) {
    let config = JmapAccountConfig {
        email: email.to_owned(),
        base_url: "https://mail.example.org".to_owned(),
        password: Some(Secret::new("pw".to_owned())),
        token: None,
        oauth: None,
        shape: mailcal_account::AccountShape::default(),
    };
    let id = config.account_id().unwrap().as_str().to_owned();
    (
        id,
        ConnectedAccount::Jmap {
            config,
            tokens: None,
            session: crate::jmap_session::JmapSession::default(),
        },
    )
}

fn support(address: Option<&str>) -> SharedStoreChoice {
    SharedStoreChoice {
        handle: SharedAccountId::try_from("f").unwrap(),
        name: "Support".to_owned(),
        address: address.map(str::to_owned),
    }
}

#[test]
fn a_chosen_shared_mailbox_dials_through_its_parent_with_mail_alone_and_its_own_address() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_store(
        &parent,
        SharedUse::Mail,
        support(Some("support@example.org")),
        true
    ));

    let [(child, _)] = registry
        .shared_stores(&parent, SharedUse::Mail)
        .try_into()
        .unwrap();
    assert!(registry.contains(child.as_str()));
    assert_eq!(
        registry
            .shared_parent(child.as_str())
            .map(|(parent, _)| parent),
        Some(parent.clone())
    );
    let dial = registry
        .dial(child.as_str())
        .expect("dialled through its parent");
    assert_eq!(dial.shared_handle().map(SharedAccountId::as_str), Some("f"));
    assert_eq!(dial.label(), "support@example.org");
    assert!(
        dial.capabilities()
            .contains(mailcal_account::Capability::Mail)
    );
    assert!(
        !dial
            .capabilities()
            .contains(mailcal_account::Capability::Calendar)
    );
    let AccountDial::Jmap { config, .. } = dial else {
        panic!("a JMAP account's shared mailbox dials as JMAP");
    };
    assert_eq!(config.email, "alice@example.org", "signs in as its parent");

    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert_eq!(placeholder.id, child);
    assert_eq!(placeholder.identity.email, "support@example.org");
    assert!(placeholder.uses_mail);
}

#[test]
fn a_shared_mailbox_without_an_address_sends_as_its_parent() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, SharedUse::Mail, support(None), true);
    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert_eq!(placeholder.identity.email, "alice@example.org");
}

#[test]
fn choosing_a_shared_mailbox_pins_its_parents_id_and_choosing_it_twice_changes_nothing() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_store(&parent, SharedUse::Mail, support(None), true));
    assert!(!registry.set_shared_store(&parent, SharedUse::Mail, support(None), true));
    let shape = registry.shape(&parent).unwrap();
    assert_eq!(
        shape.id.as_ref().map(engine_api::AccountId::as_str),
        Some(parent.as_str())
    );
    assert_eq!(shape.shared_mailboxes.len(), 1);
    let stored = registry.config_toml(&parent).unwrap().unwrap();
    assert!(stored.contains("[[shared_mailbox]]"), "{stored}");

    assert!(registry.set_shared_store(&parent, SharedUse::Mail, support(None), false));
    assert_eq!(registry.shared_stores(&parent, SharedUse::Mail), []);
}

#[test]
fn removing_the_parent_takes_its_shared_mailboxes_out_of_the_registry() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, SharedUse::Mail, support(None), true);
    let [(child, _)] = registry
        .shared_stores(&parent, SharedUse::Mail)
        .try_into()
        .unwrap();
    registry.remove(&parent);
    assert!(!registry.contains(child.as_str()));
    assert!(registry.dial(child.as_str()).is_none());
}

#[test]
fn a_chosen_shared_calendar_dials_through_its_parent_with_calendar_alone() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    let group = support(Some("support@example.org"));
    // The same store chosen for both uses: two accounts, each opening one use.
    registry.set_shared_store(&parent, SharedUse::Mail, group.clone(), true);
    registry.set_shared_store(&parent, SharedUse::Calendar, group, true);

    let [(calendar, _)] = registry
        .shared_stores(&parent, SharedUse::Calendar)
        .try_into()
        .unwrap();
    let [(mailbox, _)] = registry
        .shared_stores(&parent, SharedUse::Mail)
        .try_into()
        .unwrap();
    assert_ne!(calendar, mailbox);
    let (owner, child) = registry.shared_parent(calendar.as_str()).unwrap();
    assert_eq!((owner, child.kind), (parent.clone(), SharedUse::Calendar));

    let dial = registry
        .dial(calendar.as_str())
        .expect("dialled through its parent");
    assert_eq!(dial.shared_handle().map(SharedAccountId::as_str), Some("f"));
    let uses: Vec<_> = dial.capabilities().iter().collect();
    assert_eq!(uses, [mailcal_account::Capability::Calendar]);

    let placeholders = registry.shared_placeholders(&parent);
    let calendar_row = placeholders
        .iter()
        .find(|placeholder| placeholder.id == calendar)
        .unwrap();
    assert!(
        !calendar_row.uses_mail,
        "a shared calendar is in no mail surface"
    );
    assert_eq!(calendar_row.identity.email, "support@example.org");
    // A push watch is a mailbox's alone.
    assert!(registry.imap_store(calendar.as_str()).is_none());
}

#[test]
fn an_imap_account_without_caldav_opens_no_shared_calendar() {
    let registry = AccountRegistry::new();
    let config: mailcal_account::AccountConfig = toml::from_str(
        "[imap]\naddr = \"imap.example.org:993\"\nserver_name = \"imap.example.org\"\n\
         username = \"alice@example.org\"\npassword = \"pw\"\n",
    )
    .unwrap();
    let id = config.account_id().unwrap().as_str().to_owned();
    let entry = ConnectedAccount::Imap {
        connections: mailcal_account::ImapConnections::new(),
        config,
        tokens: None,
        talk: crate::talk_offer::TalkOffer::default(),
    };
    let _registered = registry.pre_register(id.clone(), entry);
    registry.set_shared_store(&id, SharedUse::Calendar, support(None), true);
    let [(child, _)] = registry
        .shared_stores(&id, SharedUse::Calendar)
        .try_into()
        .unwrap();
    assert!(registry.dial(child.as_str()).is_none());
}
