//! The links on an account's page (`docs/accounts.md` rule 12): a use a link fills drawn as the
//! account it comes from, the pickers only for slots nothing fills, and the accounts taking a use
//! from this one.

use AccountCapability::{Calendar, Contacts, Mail};
use CapabilityState::{Off, On};
use ReplyFallbackChoice::Ask;

use super::tests::{IDLE, applies, entry, idle, linked, mailbox_only, switch};
use crate::{
    AccountCapability, AccountEntry, AccountKind, AccountLinkedFrom, AccountPage, AccountUseBlock,
    CapabilityState, ControlState, LinkSlot, LinkedAccount, ReplyFallbackChoice,
    account_registry::Toggle,
    account_work::{Hold, PageWork},
};

#[test]
fn a_use_a_link_fills_is_the_account_it_comes_from_and_no_switch() {
    let mut account = mailbox_only();
    account.links.calendar = Some(linked("cal@cloud.example"));
    let page = super::page(
        &account,
        Hold::default(),
        None,
        Vec::new(),
        None,
        None,
        Ask,
        |capability, on| match (capability, on) {
            (Calendar | Contacts, true) => Toggle::Refused(AccountUseBlock::NeedsServer),
            _ => applies(capability, on),
        },
    );
    let calendar = switch(&page, Calendar);
    assert_eq!(calendar.linked, Some(linked("cal@cloud.example")));
    assert_eq!(
        (calendar.on, calendar.blocked, calendar.control),
        (false, None, IDLE),
        "nothing to refuse: it is not a switch"
    );
    let contacts = switch(&page, Contacts);
    assert_eq!(contacts.linked, None);
    assert_eq!(contacts.blocked, Some(AccountUseBlock::NeedsServer));
}

#[test]
fn a_use_the_account_has_itself_is_its_switch_whatever_a_link_says() {
    let mut account = entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, On), (Contacts, Off)],
    );
    account.links.calendar = Some(linked("cal@cloud.example"));
    assert_eq!(switch(&idle(&account), Calendar).linked, None);
}

#[test]
fn the_accounts_taking_a_use_say_what_they_take_and_what_they_would_be_left_without() {
    let mut calendar = entry(AccountKind::Dav, &[(Calendar, On), (Contacts, On)]);
    calendar.id = "cal@cloud.example".to_owned();
    calendar.linked_from = vec![
        linked("a@example.org"),
        linked("b@example.org"),
        linked("c@example.org"),
    ];
    let mut a = mailbox_only();
    a.id = "a@example.org".to_owned();
    a.links.calendar = Some(linked("cal@cloud.example"));
    a.links.contacts = Some(linked("cal@cloud.example"));
    let mut b = mailbox_only();
    b.id = "b@example.org".to_owned();
    b.links.contacts = Some(linked("cal@cloud.example"));
    // Listed because this calendar names it as its mail account; it takes nothing from here.
    let mut c = mailbox_only();
    c.id = "c@example.org".to_owned();
    let entries = [calendar.clone(), a, b, c];

    let taken = super::linked_from(&calendar, &entries, |account, capability| {
        account == "a@example.org" && capability == Contacts
    });
    assert_eq!(
        taken,
        [
            AccountLinkedFrom {
                account: linked("a@example.org"),
                uses: vec![Calendar, Contacts],
                leaves_without: vec![Calendar],
                stop: IDLE,
            },
            AccountLinkedFrom {
                account: linked("b@example.org"),
                uses: vec![Contacts],
                leaves_without: vec![Contacts],
                stop: IDLE,
            },
        ]
    );
}

#[test]
fn stopping_a_link_runs_on_its_row_and_holds_the_rest() {
    let calendar = entry(AccountKind::Dav, &[(Calendar, On)]);
    let row = |from: &str| AccountLinkedFrom {
        account: linked(from),
        uses: vec![Calendar],
        leaves_without: Vec::new(),
        stop: IDLE,
    };
    let page = super::page(
        &calendar,
        Hold {
            busy: true,
            own: vec![PageWork::Unlink {
                from: "a@example.org".to_owned(),
            }],
        },
        None,
        vec![row("a@example.org"), row("b@example.org")],
        None,
        None,
        Ask,
        applies,
    );
    assert_eq!(
        page.linked_from[0].stop,
        ControlState {
            enabled: false,
            running: true,
        }
    );
    assert_eq!(
        page.linked_from[1].stop,
        ControlState {
            enabled: false,
            running: false,
        }
    );
}

#[test]
fn a_picker_is_offered_only_for_a_slot_the_account_can_hold_and_nothing_fills() {
    let mut account = mailbox_only();
    assert_eq!(idle(&account).links, []);

    account.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    let links = idle(&account).links;
    assert_eq!(links.len(), 1);
    assert_eq!(
        (links[0].slot, links[0].options.len(), links[0].selected),
        (LinkSlot::Calendar, 2, None)
    );
    assert_eq!(links[0].control, IDLE);

    account.links.calendar = Some(linked("b@cloud.example"));
    assert_eq!(
        idle(&account).links,
        [],
        "a filled slot is said on its use's row"
    );
}

/// A calendar account's page while `senders` mail accounts take its calendar.
fn calendar_taken_by(account: &AccountEntry, senders: &[&str]) -> AccountPage {
    let linked_from = senders
        .iter()
        .map(|sender| crate::AccountLinkedFrom {
            account: linked(sender),
            uses: vec![Calendar],
            leaves_without: Vec::new(),
            stop: IDLE,
        })
        .collect();
    super::page(
        account,
        Hold::default(),
        None,
        linked_from,
        None,
        None,
        Ask,
        applies,
    )
}

#[test]
fn a_calendars_mail_picker_is_offered_only_while_two_or_more_take_it_or_none_is_named() {
    let mut calendar = entry(AccountKind::Dav, &[(Calendar, On)]);
    calendar.link_candidates.mail = vec![linked("a@example.org"), linked("b@example.org")];
    let unnamed = calendar_taken_by(&calendar, &[]);
    assert_eq!(unnamed.links[0].slot, LinkSlot::Mail);
    assert_eq!(unnamed.links[0].selected, None);

    calendar.links.mail = Some(linked("a@example.org"));
    assert_eq!(
        calendar_taken_by(&calendar, &["a@example.org"]).links,
        [],
        "one mail account is the one it sends through, said on its Linked to row"
    );
    let shared = calendar_taken_by(&calendar, &["a@example.org", "b@example.org"]);
    assert_eq!(
        (shared.links[0].slot, shared.links[0].selected),
        (LinkSlot::Mail, Some(0))
    );
}

#[test]
fn a_linked_account_missing_from_the_candidates_is_still_shown() {
    let mut calendar = entry(AccountKind::Dav, &[(Calendar, On)]);
    calendar.links.mail = Some(linked("gone@example.org"));
    let links = calendar_taken_by(&calendar, &["a@example.org", "b@example.org"]).links;
    assert_eq!(links[0].options, [linked("gone@example.org")]);
    assert_eq!(links[0].selected, Some(0));
}

#[test]
fn a_linked_account_is_matched_by_id_not_by_how_it_reads() {
    let mut calendar = entry(AccountKind::Dav, &[(Calendar, On)]);
    calendar.link_candidates.mail = vec![linked("a@example.org"), linked("b@example.org")];
    calendar.links.mail = Some(LinkedAccount {
        address: "b-renamed@example.org".to_owned(),
        ..linked("b@example.org")
    });
    let links = calendar_taken_by(&calendar, &["a@example.org", "b@example.org"]).links;
    assert_eq!(links[0].options.len(), 2, "listed once");
    assert_eq!(links[0].selected, Some(1));
}

#[test]
fn a_suggestion_is_offered_only_where_nothing_is_linked() {
    let mut account = mailbox_only();
    account.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    account.link_candidates.suggested = vec!["b@cloud.example".to_owned()];
    assert_eq!(idle(&account).links[0].suggested, Some(1));

    account.links.calendar = Some(linked("a@cloud.example"));
    assert_eq!(
        idle(&account).links,
        [],
        "a link already made is not second-guessed"
    );
}
