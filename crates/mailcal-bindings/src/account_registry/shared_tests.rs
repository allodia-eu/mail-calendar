//! A shared store has no entry of its own: it is dialled, watched and removed through the
//! account whose sign-in opens it, opening what it holds that the account opens.

use engine_api::SharedAccountId;
use mailcal_account::{
    Capability, JmapAccountConfig, Secret, SharedStoreChoice, SharedUse, SharedUses,
};

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

fn store(address: Option<&str>, holds: SharedUses) -> SharedStoreChoice {
    SharedStoreChoice {
        handle: SharedAccountId::try_from("f").unwrap(),
        name: "Support".to_owned(),
        address: address.map(str::to_owned),
        holds,
    }
}

fn support(address: Option<&str>) -> SharedStoreChoice {
    store(address, SharedUses::of(SharedUse::Mail))
}

fn group() -> SharedUses {
    SharedUses::of(SharedUse::Mail).with(SharedUse::Calendar)
}

/// The ids of the stores `parent` is used for, in the order chosen.
fn chosen(registry: &AccountRegistry, parent: &str) -> Vec<engine_api::AccountId> {
    registry
        .shared_children(parent)
        .into_iter()
        .map(|child| child.id)
        .collect()
}

#[test]
fn a_chosen_shared_mailbox_dials_through_its_parent_with_mail_alone_and_its_own_address() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_store(&parent, support(Some("support@example.org")), true));

    let [child] = chosen(&registry, &parent).try_into().unwrap();
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
    registry.set_shared_store(&parent, support(None), true);
    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert_eq!(placeholder.identity.email, "alice@example.org");
}

#[test]
fn choosing_a_shared_mailbox_pins_its_parents_id_and_choosing_it_twice_changes_nothing() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    assert!(registry.set_shared_store(&parent, support(None), true));
    assert!(!registry.set_shared_store(&parent, support(None), true));
    let shape = registry.shape(&parent).unwrap();
    assert_eq!(
        shape.id.as_ref().map(engine_api::AccountId::as_str),
        Some(parent.as_str())
    );
    assert_eq!(shape.shared_stores.len(), 1);
    let stored = registry.config_toml(&parent).unwrap().unwrap();
    assert!(stored.contains("[[shared_store]]"), "{stored}");

    assert!(registry.set_shared_store(&parent, support(None), false));
    assert_eq!(chosen(&registry, &parent), []);
}

#[test]
fn removing_the_parent_takes_its_shared_mailboxes_out_of_the_registry() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, support(None), true);
    let [child] = chosen(&registry, &parent).try_into().unwrap();
    registry.remove(&parent);
    assert!(!registry.contains(child.as_str()));
    assert!(registry.dial(child.as_str()).is_none());
}

#[test]
fn a_store_holding_mail_and_calendars_is_one_account_opening_both() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, store(Some("support@example.org"), group()), true);

    let [child] = chosen(&registry, &parent).try_into().unwrap();
    let dial = registry
        .dial(child.as_str())
        .expect("dialled through its parent");
    assert_eq!(dial.shared_handle().map(SharedAccountId::as_str), Some("f"));
    let uses: Vec<_> = dial.capabilities().iter().collect();
    assert_eq!(uses, [Capability::Mail, Capability::Calendar]);
    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert!(placeholder.uses_mail);
}

#[test]
fn a_store_holding_calendars_alone_is_in_no_mail_surface() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    let dave = store(
        Some("dave@example.org"),
        SharedUses::of(SharedUse::Calendar),
    );
    registry.set_shared_store(&parent, dave, true);

    let [child] = chosen(&registry, &parent).try_into().unwrap();
    let dial = registry.dial(child.as_str()).unwrap();
    let uses: Vec<_> = dial.capabilities().iter().collect();
    assert_eq!(uses, [Capability::Calendar]);
    let [placeholder] = registry.shared_placeholders(&parent).try_into().unwrap();
    assert!(!placeholder.uses_mail);
    assert_eq!(placeholder.identity.email, "dave@example.org");
    // A push watch is a mailbox's alone.
    assert!(registry.imap_store(child.as_str()).is_none());
}

#[test]
fn choosing_a_store_again_gains_what_it_holds_now_and_never_loses_any() {
    let registry = AccountRegistry::new();
    let (parent, entry) = jmap("alice@example.org");
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, support(None), true);
    assert!(registry.set_shared_store(&parent, store(None, group()), true));
    assert!(!registry.set_shared_store(&parent, support(None), true));
    let [child] = registry.shared_children(&parent).try_into().unwrap();
    assert_eq!(child.choice.holds, group());
}

#[test]
fn a_store_opens_only_what_its_parent_is_used_for() {
    let registry = AccountRegistry::new();
    let (parent, mut entry) = jmap("alice@example.org");
    if let ConnectedAccount::Jmap { config, .. } = &mut entry {
        config.shape.capabilities = Some([Capability::Calendar].into_iter().collect());
    }
    let _registered = registry.pre_register(parent.clone(), entry);
    registry.set_shared_store(&parent, store(None, group()), true);
    registry.set_shared_store(
        &parent,
        SharedStoreChoice {
            handle: SharedAccountId::try_from("g").unwrap(),
            ..support(None)
        },
        true,
    );
    let [both, mail] = chosen(&registry, &parent).try_into().unwrap();
    let uses: Vec<_> = registry
        .dial(both.as_str())
        .unwrap()
        .capabilities()
        .iter()
        .collect();
    assert_eq!(uses, [Capability::Calendar]);
    assert!(
        registry.dial(mail.as_str()).is_none(),
        "nothing it holds is opened"
    );
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
    registry.set_shared_store(&id, store(None, SharedUses::of(SharedUse::Calendar)), true);
    let [child] = chosen(&registry, &id).try_into().unwrap();
    assert!(registry.dial(child.as_str()).is_none());
}
