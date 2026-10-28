//! A shared mailbox has no entry of its own: it is dialled, watched and removed through the
//! account whose sign-in opens it.

use engine_api::SharedMailboxId;
use mailcal_account::{JmapAccountConfig, Secret, SharedMailboxChoice};

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

fn support(address: Option<&str>) -> SharedMailboxChoice {
    SharedMailboxChoice {
        handle: SharedMailboxId::try_from("f").unwrap(),
        name: "Support".to_owned(),
        address: address.map(str::to_owned),
    }
}

#[test]
fn a_chosen_shared_mailbox_dials_through_its_parent_with_mail_alone_and_its_own_address() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_mailbox(&parent, support(Some("support@example.org")), true));

    let [(child, _)] = registry.shared_mailboxes(&parent).try_into().unwrap();
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
    assert_eq!(dial.shared_handle().map(SharedMailboxId::as_str), Some("f"));
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
    registry.set_shared_mailbox(&parent, support(None), true);
    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert_eq!(placeholder.identity.email, "alice@example.org");
}

#[test]
fn choosing_a_shared_mailbox_pins_its_parents_id_and_choosing_it_twice_changes_nothing() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_mailbox(&parent, support(None), true));
    assert!(!registry.set_shared_mailbox(&parent, support(None), true));
    let shape = registry.shape(&parent).unwrap();
    assert_eq!(
        shape.id.as_ref().map(engine_api::AccountId::as_str),
        Some(parent.as_str())
    );
    assert_eq!(shape.shared_mailboxes.len(), 1);
    let stored = registry.config_toml(&parent).unwrap().unwrap();
    assert!(stored.contains("[[shared_mailbox]]"), "{stored}");

    assert!(registry.set_shared_mailbox(&parent, support(None), false));
    assert_eq!(registry.shared_mailboxes(&parent), []);
}

#[test]
fn removing_the_parent_takes_its_shared_mailboxes_out_of_the_registry() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_mailbox(&parent, support(None), true);
    let [(child, _)] = registry.shared_mailboxes(&parent).try_into().unwrap();
    registry.remove(&parent);
    assert!(!registry.contains(child.as_str()));
    assert!(registry.dial(child.as_str()).is_none());
}
