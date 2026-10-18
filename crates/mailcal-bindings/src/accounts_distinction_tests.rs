use std::collections::{BTreeMap, BTreeSet};

use mailcal_account::{AccountLinks, Capability};

use crate::{
    AccountDistinction, AccountEntry, AccountKind,
    accounts_view::{
        AccountFacts, entries,
        tests::{cloud, id, mailbox, set, to_cloud},
    },
};

fn listed(facts: &[AccountFacts]) -> Vec<AccountEntry> {
    entries(facts, &BTreeSet::new(), &BTreeMap::new())
}

/// The calendar split off from `mailbox`: the same person's login, at a server of its own.
fn split_off(links: AccountLinks) -> AccountFacts {
    AccountFacts {
        id: "alice@example.org@dav:cloud.example".to_owned(),
        address: "Alice@Example.org".to_owned(),
        ..cloud(links)
    }
}

fn to_split() -> AccountLinks {
    AccountLinks {
        calendar: id("alice@example.org@dav:cloud.example"),
        contacts: id("alice@example.org@dav:cloud.example"),
        mail: None,
    }
}

fn kind(kind: AccountKind) -> Option<AccountDistinction> {
    Some(AccountDistinction::Kind { kind })
}

#[test]
fn a_mailbox_and_the_calendar_split_off_from_it_are_told_apart_by_kind() {
    let listed = listed(&[mailbox(to_split()), split_off(AccountLinks::default())]);
    let (mail, calendar) = (&listed[0], &listed[1]);

    assert_eq!(mail.distinction, kind(AccountKind::Imap));
    assert_eq!(calendar.distinction, kind(AccountKind::Dav));
    for link in [
        &mail.links.calendar,
        &mail.links.contacts,
        &calendar.links.mail,
    ] {
        let link = link.as_ref().expect("both ends hold the link");
        let expected = if link.id == mail.id {
            &mail.distinction
        } else {
            &calendar.distinction
        };
        assert_eq!(&link.distinction, expected, "{}", link.id);
    }
    assert_eq!(calendar.linked_from[0].distinction, kind(AccountKind::Imap));
    assert_eq!(
        mail.link_candidates.calendar[0].distinction,
        kind(AccountKind::Dav)
    );
}

#[test]
fn accounts_of_one_kind_and_address_are_told_apart_by_their_server() {
    let elsewhere = AccountFacts {
        id: "alice@example.org@dav:other.example".to_owned(),
        server: Some("other.example".to_owned()),
        ..split_off(AccountLinks::default())
    };
    let listed = listed(&[
        mailbox(to_split()),
        split_off(AccountLinks::default()),
        elsewhere,
    ]);

    assert_eq!(listed[0].distinction, kind(AccountKind::Imap));
    let hosts: Vec<_> = listed[1..]
        .iter()
        .map(|entry| entry.distinction.clone())
        .collect();
    assert_eq!(
        hosts,
        [
            Some(AccountDistinction::Server {
                host: "cloud.example".to_owned()
            }),
            Some(AccountDistinction::Server {
                host: "other.example".to_owned()
            }),
        ]
    );
    let offered: Vec<_> = listed[0]
        .link_candidates
        .calendar
        .iter()
        .map(|candidate| candidate.distinction.clone())
        .collect();
    assert_eq!(offered, hosts, "the picker tells the two calendars apart");
}

#[test]
fn an_address_no_other_account_has_is_read_alone() {
    let listed = listed(&[mailbox(to_cloud()), cloud(AccountLinks::default())]);

    assert!(listed.iter().all(|entry| entry.distinction.is_none()));
    let link = listed[0].links.calendar.as_ref().expect("the link holds");
    assert_eq!(link.distinction, None);
}

#[test]
fn a_namesake_of_the_same_kind_with_no_server_to_name_is_left_unmarked() {
    let google = |id: &str| AccountFacts {
        id: id.to_owned(),
        kind: AccountKind::Google,
        server: None,
        chosen: set(&[Capability::Mail]),
        ..mailbox(AccountLinks::default())
    };

    let listed = listed(&[google("a"), google("b")]);

    assert!(listed.iter().all(|entry| entry.distinction.is_none()));
}
