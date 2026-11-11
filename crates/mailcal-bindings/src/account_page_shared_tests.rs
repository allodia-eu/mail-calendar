use engine_api::{SearchDomain, SharedAccount, SharedAccountId, StoreDomains};
use mailcal_account::SharedStoreChoice;
use mailcal_app::SharedStoreOffer;

use super::{Asked, SharedAnswers};
use crate::{
    ControlState, SharedMailboxesNote,
    account_work::{Hold, PageWork},
};

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

fn listed(handle: &str, name: &str, address: Option<&str>) -> SharedAccount {
    let mailbox = SharedAccount::new(
        SharedAccountId::try_from(handle).unwrap(),
        name,
        StoreDomains::of(SearchDomain::Mail),
    );
    match address {
        Some(address) => mailbox.with_address(address),
        None => mailbox,
    }
}

fn chosen(handle: &str, address: &str) -> SharedStoreChoice {
    SharedStoreChoice {
        handle: SharedAccountId::try_from(handle).unwrap(),
        name: address.to_owned(),
        address: Some(address.to_owned()),
        holds: mailcal_account::SharedUses::of(mailcal_account::SharedUse::Mail),
    }
}

fn asked(live: bool) -> Asked {
    let mail = mailcal_account::SharedUses::of(mailcal_account::SharedUse::Mail);
    Asked {
        uses: [mailcal_account::Capability::Mail].into_iter().collect(),
        live: if live {
            mail
        } else {
            mailcal_account::SharedUses::default()
        },
    }
}

#[test]
fn the_server_is_asked_once_and_again_only_when_the_account_changed_or_its_page_opened() {
    let answers = SharedAnswers::default();
    let now = asked(false);
    let first = answers.ask("a", &now, false).expect("nothing answered yet");
    assert_eq!(answers.ask("a", &now, true), None, "one ask at a time");
    answers.answered("a", first, Ok(SharedStoreOffer::NotConnected));

    assert_eq!(
        answers.ask("a", &now, false),
        None,
        "a page drawn again is no reason to ask: its answer raised the signal that drew it"
    );
    let connected = answers
        .ask("a", &asked(true), false)
        .expect("a mailbox that connected can now be asked");
    answers.answered("a", connected, Ok(SharedStoreOffer::ByAddress));
    assert!(
        answers.ask("a", &asked(true), true).is_some(),
        "its page opened"
    );
    assert!(
        answers.ask("b", &now, false).is_some(),
        "each account is asked for itself"
    );
}

#[test]
fn an_older_answer_landing_late_does_not_replace_a_newer_one() {
    let answers = SharedAnswers::default();
    let old = answers.ask("a", &asked(true), false).unwrap();
    answers.answered("a", old, Ok(SharedStoreOffer::ByAddress));
    let new = answers.ask("a", &asked(true), true).unwrap();
    answers.answered("a", new, Ok(SharedStoreOffer::Listed(Vec::new())));
    answers.answered("a", old, Ok(SharedStoreOffer::Unsupported));
    assert_eq!(
        answers.offer("a"),
        Some(Ok(SharedStoreOffer::Listed(Vec::new())))
    );
}

#[test]
fn an_answer_for_an_account_forgotten_meanwhile_is_dropped() {
    let answers = SharedAnswers::default();
    let ticket = answers.ask("a", &asked(true), false).unwrap();
    answers.forget("a");
    answers.answered("a", ticket, Ok(SharedStoreOffer::ByAddress));
    assert_eq!(answers.offer("a"), None);
}

#[test]
fn a_switch_is_titled_by_its_address_and_a_withdrawn_one_says_so() {
    let answers = SharedAnswers::default();
    let offer = Ok(SharedStoreOffer::Listed(vec![
        listed("s", "Support team", Some("support@example.org")),
        listed("n", "news", None),
    ]));
    let section = answers
        .section(
            "a",
            Some(offer),
            vec![chosen("gone", "old@example.org")],
            &Hold::default(),
        )
        .expect("the server answered");
    let rows: Vec<_> = section
        .mailboxes
        .iter()
        .map(|row| {
            (
                row.title.as_str(),
                row.label.as_deref(),
                row.withdrawn,
                row.on,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("old@example.org", None, true, true),
            ("support@example.org", Some("Support team"), false, false),
            ("news", None, false, false),
        ]
    );
    assert!(section.mailboxes.iter().all(|row| row.control == FREE));
    assert_eq!(section.note, None);
    assert!(
        section.address.is_none(),
        "a server that lists them takes no address"
    );
}

#[test]
fn the_line_under_the_switches_says_what_the_server_answered() {
    let answers = SharedAnswers::default();
    let note = |offer| {
        answers
            .section("a", Some(offer), Vec::new(), &Hold::default())
            .and_then(|section| section.note)
    };
    assert_eq!(
        note(Ok(SharedStoreOffer::Listed(Vec::new()))),
        Some(SharedMailboxesNote::NoneShared)
    );
    assert_eq!(
        note(Ok(SharedStoreOffer::NotConnected)),
        Some(SharedMailboxesNote::NotConnected)
    );
    assert_eq!(note(Err(())), Some(SharedMailboxesNote::Unreachable));
    assert!(
        answers
            .section(
                "a",
                Some(Ok(SharedStoreOffer::Unsupported)),
                Vec::new(),
                &Hold::default()
            )
            .is_none(),
        "an account that cannot open one has no section that says nothing"
    );
    assert!(
        answers
            .section("a", None, Vec::new(), &Hold::default())
            .is_none(),
        "drawn once the server has answered"
    );
}

#[test]
fn a_running_switch_and_add_are_drawn_where_they_go_and_hold_the_rest() {
    let answers = SharedAnswers::default();
    let offer = Ok(SharedStoreOffer::Listed(vec![
        listed("s", "s", Some("support@example.org")),
        listed("t", "t", Some("team@example.org")),
    ]));
    let hold = Hold {
        busy: true,
        own: vec![PageWork::Shared {
            handle: "s".to_owned(),
            on: true,
        }],
    };
    let section = answers
        .section("a", Some(offer), Vec::new(), &hold)
        .unwrap();
    assert!(section.mailboxes[0].on && section.mailboxes[0].control.running);
    assert!(!section.mailboxes[1].control.enabled && !section.mailboxes[1].control.running);

    let adding = Hold {
        busy: true,
        own: vec![PageWork::AddShared("help@example.org".to_owned())],
    };
    let field = answers
        .section(
            "a",
            Some(Ok(SharedStoreOffer::ByAddress)),
            Vec::new(),
            &adding,
        )
        .and_then(|section| section.address)
        .expect("a server that opens one by address takes one");
    assert_eq!(field.text, "help@example.org");
    assert!(field.add.running);

    answers.missed("a", Some("typo@example.org".to_owned()));
    let field = answers
        .section(
            "a",
            Some(Ok(SharedStoreOffer::ByAddress)),
            Vec::new(),
            &Hold::default(),
        )
        .and_then(|section| section.address)
        .unwrap();
    assert_eq!(
        (field.text.as_str(), field.add),
        ("typo@example.org", FREE),
        "an address nothing answered to stays to be corrected"
    );
}
