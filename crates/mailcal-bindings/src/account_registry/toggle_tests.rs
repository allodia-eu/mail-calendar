//! What switching a use on or off comes to: the one decision `set_use` acts on and an account's
//! page reads before the switch is pressed, so the two are asked of the same accounts here.

use std::{collections::BTreeMap, sync::Arc};

use mailcal_account::{Capabilities, Capability};

use super::{Toggle, UseChange};
use crate::{
    AccountUseBlock, AccountUseStatus, MailcalError, account_registry::AccountRegistry,
    account_work::Hold,
};

const MICROSOFT_ID: &str = "alice@work.example@graph.microsoft.com";
const IMAP_ID: &str = "alice@example.org@localhost";

/// A Microsoft work account used for `uses`, granted `granted`.
fn microsoft(granted: &[&str], uses: &str) -> Arc<AccountRegistry> {
    let config = mailcal_account::MicrosoftConfig {
        email: "alice@work.example".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: Vec::new(),
        refresh_token: mailcal_account::Secret::new("refresh".to_owned()),
        granted_scopes: Some(granted.iter().map(|&scope| scope.to_owned()).collect()),
        affiliation: None,
        shape: mailcal_account::AccountShape::read(&format!("capabilities = {uses}")).unwrap(),
    };
    let id = config.account_id().unwrap();
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .unwrap();
    let registry = AccountRegistry::new();
    registry
        .pre_register(
            id.as_str().to_owned(),
            crate::ConnectedAccount::Microsoft { config, tokens },
        )
        .commit();
    registry
}

/// An IMAP mailbox with no calendar or address-book server.
fn mailbox_only() -> Arc<AccountRegistry> {
    let config = mailcal_account::load_str(
        r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#,
    )
    .unwrap();
    let registry = AccountRegistry::new();
    registry
        .pre_register(
            IMAP_ID.to_owned(),
            crate::ConnectedAccount::imap_account(config, None),
        )
        .commit();
    registry
}

const EVERY_SCOPE: &[&str] = &[
    "Mail.ReadWrite",
    "Calendars.ReadWrite",
    "Contacts.ReadWrite",
    "People.Read",
    "OnlineMeetings.ReadWrite",
];

fn toggle(registry: &AccountRegistry, id: &str, capability: Capability, on: bool) -> Toggle {
    registry
        .toggle(id, capability, on, &Capabilities::default())
        .expect("registered")
}

#[test]
fn the_last_use_cannot_go_and_a_use_with_no_server_says_so() {
    let registry = mailbox_only();
    assert_eq!(
        toggle(&registry, IMAP_ID, Capability::Mail, false),
        Toggle::Refused(AccountUseBlock::LastUse)
    );
    assert_eq!(
        toggle(&registry, IMAP_ID, Capability::Calendar, true),
        Toggle::Refused(AccountUseBlock::NeedsServer)
    );
    // `set_use` acts on the same answers.
    assert!(matches!(
        registry.set_use(IMAP_ID, Capability::Mail, false, &Capabilities::default()),
        Err(MailcalError::Config(reason)) if reason == AccountUseBlock::LastUse.reason()
    ));
    assert!(matches!(
        registry.set_use(
            IMAP_ID,
            Capability::Calendar,
            true,
            &Capabilities::default()
        ),
        Ok(UseChange::NeedsEndpoint)
    ));
    assert_eq!(
        toggle(&registry, IMAP_ID, Capability::Mail, true),
        Toggle::Unchanged
    );
}

#[test]
fn colleagues_wait_for_contacts_and_shared_mailboxes_for_mail() {
    let registry = microsoft(EVERY_SCOPE, r#"["mail", "calendar"]"#);
    assert_eq!(
        toggle(&registry, MICROSOFT_ID, Capability::Colleagues, true),
        Toggle::Refused(AccountUseBlock::NeedsContacts)
    );
    let registry = microsoft(EVERY_SCOPE, r#"["calendar", "contacts"]"#);
    assert_eq!(
        toggle(&registry, MICROSOFT_ID, Capability::SharedMailboxes, true),
        Toggle::Refused(AccountUseBlock::NeedsMail)
    );
}

#[test]
fn switching_contacts_off_takes_colleagues_with_it() {
    let registry = microsoft(EVERY_SCOPE, r#"["mail", "contacts", "colleagues"]"#);
    let Toggle::Applies { next } = toggle(&registry, MICROSOFT_ID, Capability::Contacts, false)
    else {
        panic!("contacts can go while mail stays");
    };
    assert!(next.contains(Capability::Mail));
    assert!(!next.contains(Capability::Contacts) && !next.contains(Capability::Colleagues));
}

#[test]
fn a_grant_that_lacks_a_use_asks_the_provider() {
    let registry = microsoft(&["Calendars.ReadWrite"], r#"["calendar"]"#);
    assert_eq!(
        toggle(&registry, MICROSOFT_ID, Capability::Meetings, true),
        Toggle::AsksProvider
    );
    assert!(matches!(
        registry.set_use(
            MICROSOFT_ID,
            Capability::Meetings,
            true,
            &Capabilities::default()
        ),
        Ok(UseChange::NeedsConsent)
    ));
}

#[test]
fn a_use_chosen_and_not_granted_asks_the_provider_again() {
    // The grant came back without it: an administrator withheld it.
    let registry = microsoft(&["Calendars.ReadWrite"], r#"["calendar", "meetings"]"#);
    assert_eq!(
        toggle(&registry, MICROSOFT_ID, Capability::Meetings, true),
        Toggle::AsksProvider
    );
    assert!(matches!(
        registry.set_use(
            MICROSOFT_ID,
            Capability::Meetings,
            true,
            &Capabilities::default()
        ),
        Ok(UseChange::NeedsConsent)
    ));
    // The grant allows it, and the provider refused it when it was opened.
    let registry = microsoft(EVERY_SCOPE, r#"["mail", "calendar"]"#);
    let refused: Capabilities = [Capability::Calendar].into_iter().collect();
    assert_eq!(
        registry.toggle(MICROSOFT_ID, Capability::Calendar, true, &refused),
        Some(Toggle::AsksProvider)
    );
    assert!(matches!(
        registry.set_use(MICROSOFT_ID, Capability::Calendar, true, &refused),
        Ok(UseChange::NeedsConsent)
    ));
    assert_eq!(
        toggle(&registry, MICROSOFT_ID, Capability::Calendar, true),
        Toggle::Unchanged,
        "granted and opened, it is already on"
    );
}

#[test]
fn the_page_draws_what_the_registry_decides() {
    let registry = microsoft(
        &["Calendars.ReadWrite"],
        r#"["mail", "calendar", "meetings"]"#,
    );
    let entry =
        crate::accounts_view::entries(&registry.facts(), &BTreeMap::new(), &BTreeMap::new())
            .remove(0);
    let page = crate::account_page::page(
        &entry,
        Hold::default(),
        None,
        Vec::new(),
        None,
        None,
        crate::ReplyFallbackChoice::Ask,
        |capability, on| toggle(&registry, MICROSOFT_ID, Capability::from(capability), on),
    );
    let switch = |capability| {
        page.uses
            .iter()
            .find(|switch| switch.capability == capability)
            .cloned()
            .expect("listed")
    };
    let colleagues = switch(crate::AccountCapability::Colleagues);
    assert_eq!(colleagues.blocked, Some(AccountUseBlock::NeedsContacts));
    assert!(!colleagues.control.enabled);
    let meetings = switch(crate::AccountCapability::Meetings);
    assert_eq!(meetings.status, Some(AccountUseStatus::Withheld));
    assert!(meetings.asks_provider && meetings.control.enabled);
}
