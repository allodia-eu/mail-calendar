use std::collections::BTreeSet;

use engine_api::AccountId;
use mailcal_account::{AccountLinks, Capabilities, Capability};

use super::{AccountFacts, entries};
use crate::{AccountCapability, AccountKind, CapabilityState};

const STANDARDS: [Capability; 3] = [Capability::Mail, Capability::Calendar, Capability::Contacts];

fn set(capabilities: &[Capability]) -> Capabilities {
    capabilities.iter().copied().collect()
}

fn id(text: &str) -> Option<AccountId> {
    Some(AccountId::try_from(text).unwrap())
}

fn mailbox(links: AccountLinks) -> AccountFacts {
    AccountFacts {
        id: "alice@imap.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        kind: AccountKind::Imap,
        offered: set(&STANDARDS),
        chosen: set(&[Capability::Mail]),
        withheld: Capabilities::default(),
        files_invitations: false,
        links,
        endpoints: None,
        calendar_addresses: Vec::new(),
    }
}

fn cloud(links: AccountLinks) -> AccountFacts {
    AccountFacts {
        id: "alice@dav:cloud.example".to_owned(),
        address: "alice".to_owned(),
        kind: AccountKind::Dav,
        offered: set(&STANDARDS),
        chosen: set(&[Capability::Calendar, Capability::Contacts]),
        withheld: Capabilities::default(),
        files_invitations: true,
        links,
        endpoints: None,
        calendar_addresses: Vec::new(),
    }
}

fn to_cloud() -> AccountLinks {
    AccountLinks {
        calendar: id("alice@dav:cloud.example"),
        contacts: id("alice@dav:cloud.example"),
        mail: None,
    }
}

fn state(entry: &crate::AccountEntry, capability: AccountCapability) -> Option<CapabilityState> {
    entry
        .uses
        .iter()
        .find(|row| row.capability == capability)
        .map(|row| row.state)
}

#[test]
fn a_mail_account_linked_to_a_calendar_reads_the_link_by_address_from_both_ends() {
    let accounts = entries(
        &[mailbox(to_cloud()), cloud(AccountLinks::default())],
        &BTreeSet::new(),
    );

    let calendar = accounts[0].links.calendar.as_ref().expect("linked");
    assert_eq!(calendar.id, "alice@dav:cloud.example");
    assert_eq!(calendar.address, "alice");
    assert!(accounts[0].links.contacts.is_some());

    // One mail account links the calendar, so it sends for it without anyone choosing.
    let sender = accounts[1].links.mail.as_ref().expect("implied sender");
    assert_eq!(sender.address, "alice@example.org");
    let from: Vec<&str> = accounts[1]
        .linked_from
        .iter()
        .map(|linked| linked.id.as_str())
        .collect();
    assert_eq!(from, ["alice@imap.example.org"]);
}

#[test]
fn a_link_the_accounts_no_longer_allow_reads_as_none() {
    // Dangling: nothing has that id.
    let dangling = entries(&[mailbox(to_cloud())], &BTreeSet::new());
    assert!(dangling[0].links.calendar.is_none());

    // The mail account has a calendar of its own now.
    let mut own = mailbox(to_cloud());
    own.chosen = set(&[Capability::Mail, Capability::Calendar]);
    let accounts = entries(&[own, cloud(AccountLinks::default())], &BTreeSet::new());
    assert!(accounts[0].links.calendar.is_none());
    assert!(accounts[0].links.contacts.is_some());

    // The target is no longer used for its calendar, or has none that can file an invitation.
    let mut contacts_only = cloud(AccountLinks::default());
    contacts_only.chosen = set(&[Capability::Contacts]);
    let accounts = entries(&[mailbox(to_cloud()), contacts_only], &BTreeSet::new());
    assert!(accounts[0].links.calendar.is_none());
    assert!(accounts[1].links.mail.is_none());

    let mut graph = cloud(AccountLinks::default());
    graph.files_invitations = false;
    let accounts = entries(&[mailbox(to_cloud()), graph], &BTreeSet::new());
    assert!(accounts[0].links.calendar.is_none());
}

#[test]
fn a_calendar_two_mailboxes_link_sends_through_the_one_it_names() {
    let mut second = mailbox(to_cloud());
    second.id = "bob@imap.example.org".to_owned();
    second.address = "bob@example.org".to_owned();

    let unnamed = entries(
        &[
            mailbox(to_cloud()),
            second.clone(),
            cloud(AccountLinks::default()),
        ],
        &BTreeSet::new(),
    );
    assert!(
        unnamed[2].links.mail.is_none(),
        "two candidates, none named"
    );
    assert_eq!(unnamed[2].linked_from.len(), 2);

    let named = entries(
        &[
            mailbox(to_cloud()),
            second,
            cloud(AccountLinks {
                mail: id("bob@imap.example.org"),
                ..AccountLinks::default()
            }),
        ],
        &BTreeSet::new(),
    );
    assert_eq!(
        named[2]
            .links
            .mail
            .as_ref()
            .map(|linked| linked.id.as_str()),
        Some("bob@imap.example.org")
    );
}

#[test]
fn each_kind_lists_what_it_can_be_used_for_in_its_state() {
    let mut graph = mailbox(AccountLinks::default());
    graph.kind = AccountKind::Microsoft;
    graph.offered = set(&Capability::ALL);
    graph.chosen = set(&[Capability::Mail, Capability::Calendar, Capability::Contacts]);
    graph.withheld = set(&[Capability::Contacts]);
    let standards = cloud(AccountLinks::default());
    let mut calendar_refused = mailbox(AccountLinks::default());
    calendar_refused.id = "carol@example.com@graph.microsoft.com".to_owned();
    calendar_refused.kind = AccountKind::Microsoft;
    calendar_refused.offered = set(&Capability::ALL);
    calendar_refused.chosen = set(&Capability::ALL);
    let refused: BTreeSet<String> = [calendar_refused.id.clone()].into();

    let accounts = entries(&[graph, standards, calendar_refused], &refused);

    assert_eq!(
        state(&accounts[0], AccountCapability::Mail),
        Some(CapabilityState::On)
    );
    assert_eq!(
        state(&accounts[0], AccountCapability::Contacts),
        Some(CapabilityState::NeedsPermission)
    );
    assert_eq!(
        state(&accounts[0], AccountCapability::Colleagues),
        Some(CapabilityState::Off)
    );
    assert_eq!(
        state(&accounts[1], AccountCapability::Mail),
        Some(CapabilityState::Off)
    );
    assert_eq!(state(&accounts[1], AccountCapability::Colleagues), None);
    assert_eq!(
        state(&accounts[2], AccountCapability::Calendar),
        Some(CapabilityState::NeedsPermission)
    );
}

#[test]
fn a_use_the_account_does_not_offer_is_not_listed_at_all() {
    // A personal Microsoft account: no organisation, so no colleagues, rather than colleagues
    // waiting on a permission no sign-in can give.
    let mut personal = mailbox(AccountLinks::default());
    personal.kind = AccountKind::Microsoft;
    personal.offered = set(&STANDARDS);
    personal.chosen = set(&STANDARDS);
    personal.withheld = set(&[Capability::Colleagues]);

    let accounts = entries(&[personal], &BTreeSet::new());

    assert_eq!(state(&accounts[0], AccountCapability::Colleagues), None);
    assert_eq!(
        state(&accounts[0], AccountCapability::Contacts),
        Some(CapabilityState::On)
    );
}

#[test]
fn each_link_offers_only_the_accounts_it_may_name() {
    let mut graph = mailbox(AccountLinks::default());
    graph.id = "bob@example.com@graph.microsoft.com".to_owned();
    graph.kind = AccountKind::Microsoft;
    graph.chosen = set(&[Capability::Mail, Capability::Calendar, Capability::Contacts]);
    let accounts = entries(
        &[
            mailbox(AccountLinks::default()),
            cloud(AccountLinks::default()),
            graph,
        ],
        &BTreeSet::new(),
    );
    let ids = |offered: &[crate::LinkedAccount]| -> Vec<String> {
        offered.iter().map(|linked| linked.id.clone()).collect()
    };

    let mail = &accounts[0].link_candidates;
    assert_eq!(
        ids(&mail.calendar),
        ["alice@dav:cloud.example"],
        "CalDAV only"
    );
    assert_eq!(
        ids(&mail.contacts),
        [
            "alice@dav:cloud.example",
            "bob@example.com@graph.microsoft.com"
        ]
    );
    assert!(mail.mail.is_empty());

    // The calendar may send through the mailbox without a calendar, not the one with its own.
    let calendar = &accounts[1].link_candidates;
    assert_eq!(ids(&calendar.mail), ["alice@imap.example.org"]);
    assert!(calendar.calendar.is_empty() && calendar.contacts.is_empty());

    let complete = &accounts[2].link_candidates;
    assert!(
        complete.calendar.is_empty() && complete.contacts.is_empty() && complete.mail.is_empty()
    );
}

/// A calendar server that schedules as the mail account's address is suggested for it, from
/// either end; one that does not is offered without being suggested.
#[test]
fn a_calendar_that_recognises_the_mail_address_is_suggested_from_both_ends() {
    let mut recognising = cloud(AccountLinks::default());
    recognising.calendar_addresses = vec!["Alice@Example.org".to_owned()];
    let accounts = entries(
        &[mailbox(AccountLinks::default()), recognising],
        &BTreeSet::new(),
    );
    assert_eq!(
        accounts[0].link_candidates.suggested,
        ["alice@dav:cloud.example"]
    );
    assert_eq!(
        accounts[1].link_candidates.suggested,
        ["alice@imap.example.org"]
    );

    let mut stranger = cloud(AccountLinks::default());
    stranger.calendar_addresses = vec!["bob@example.org".to_owned()];
    let accounts = entries(
        &[mailbox(AccountLinks::default()), stranger],
        &BTreeSet::new(),
    );
    assert_eq!(accounts[0].link_candidates.calendar.len(), 1);
    assert!(accounts[0].link_candidates.suggested.is_empty());
    assert!(accounts[1].link_candidates.suggested.is_empty());
}

/// Nothing is suggested for an address book: a CardDAV server names no addresses.
#[test]
fn an_address_book_is_never_suggested() {
    let mut contacts_only = cloud(AccountLinks::default());
    contacts_only.chosen = set(&[Capability::Contacts]);
    contacts_only.files_invitations = false;
    contacts_only.calendar_addresses = vec!["alice@example.org".to_owned()];
    let accounts = entries(
        &[mailbox(AccountLinks::default()), contacts_only],
        &BTreeSet::new(),
    );
    assert_eq!(accounts[0].link_candidates.contacts.len(), 1);
    assert!(accounts[0].link_candidates.suggested.is_empty());
}
