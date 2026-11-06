//! An account's page, decided from its entry alone: the rules every client used to keep a copy
//! of, and how work running on the page holds its controls.

use AccountCapability::{Calendar, Colleagues, Contacts, Mail, Meetings, SharedMailboxes};
use CapabilityState::{NeedsPermission, NotOffered, Off, On};

use super::{SwitchOn, page};
use crate::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountPage, AccountUse,
    AccountUseNote, AccountUseSwitch, CapabilityState, ControlState, LinkCandidates, LinkSlot,
    LinkedAccount, SignInMethod, UseProblem, account_work::PageWork,
};

const IDLE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};

fn linked(id: &str) -> LinkedAccount {
    LinkedAccount {
        id: id.to_owned(),
        address: id.to_owned(),
        distinction: None,
    }
}

fn entry(kind: AccountKind, uses: &[(AccountCapability, CapabilityState)]) -> AccountEntry {
    AccountEntry {
        id: "alice@example.org@imap.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        kind,
        sign_in: SignInMethod::Password,
        uses: uses
            .iter()
            .map(|&(capability, state)| AccountUse {
                capability,
                state,
                problem: None,
            })
            .collect(),
        links: AccountLinksView::default(),
        linked_from: Vec::new(),
        link_candidates: LinkCandidates::default(),
        endpoints: None,
        separate_server: None,
        distinction: None,
        mail_sync: None,
        nextcloud: false,
    }
}

fn mailbox_only() -> AccountEntry {
    entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, Off), (Contacts, Off)],
    )
}

/// The page with nothing running, switching any use on applying at once.
fn idle(entry: &AccountEntry) -> AccountPage {
    page(entry, None, None, |_| SwitchOn::Applies)
}

fn switch(page: &AccountPage, capability: AccountCapability) -> &AccountUseSwitch {
    page.uses
        .iter()
        .find(|switch| switch.capability == capability)
        .expect("the page lists the use")
}

#[test]
fn the_last_use_cannot_be_switched_off() {
    let page = idle(&mailbox_only());
    assert_eq!(
        switch(&page, Mail),
        &AccountUseSwitch {
            capability: Mail,
            on: true,
            control: HELD,
            note: Some(AccountUseNote::Last),
            asks_provider: false,
            confirms_off: true,
            reviews_server_change: false,
        }
    );
    let calendar = switch(&page, Calendar);
    assert_eq!(
        (calendar.on, calendar.control, calendar.note),
        (false, IDLE, None)
    );
}

#[test]
fn video_meetings_are_never_the_last_use_and_switch_off_without_asking() {
    let page = idle(&entry(
        AccountKind::Dav,
        &[(Calendar, On), (Contacts, Off), (Meetings, On)],
    ));
    assert_eq!(switch(&page, Calendar).note, Some(AccountUseNote::Last));
    let meetings = switch(&page, Meetings);
    assert_eq!((meetings.control, meetings.note), (IDLE, None));
    assert!(!meetings.confirms_off);
    assert!(switch(&page, Calendar).confirms_off);
}

#[test]
fn colleagues_wait_for_contacts_and_shared_mailboxes_for_mail() {
    let mut account = entry(
        AccountKind::Microsoft,
        &[
            (Mail, Off),
            (SharedMailboxes, Off),
            (Calendar, On),
            (Contacts, Off),
            (Colleagues, Off),
        ],
    );
    let page_now = idle(&account);
    assert_eq!(
        (
            switch(&page_now, Colleagues).control,
            switch(&page_now, Colleagues).note
        ),
        (HELD, Some(AccountUseNote::NeedsContacts))
    );
    assert_eq!(
        (
            switch(&page_now, SharedMailboxes).control,
            switch(&page_now, SharedMailboxes).note
        ),
        (HELD, Some(AccountUseNote::NeedsMail))
    );

    account.uses[0].state = On;
    account.uses[3].state = On;
    let page_now = idle(&account);
    assert_eq!(switch(&page_now, Colleagues).control, IDLE);
    assert_eq!(switch(&page_now, SharedMailboxes).control, IDLE);
}

#[test]
fn a_use_waiting_on_permission_is_drawn_off_and_switching_it_on_asks() {
    let page = idle(&entry(
        AccountKind::Google,
        &[(Mail, Off), (Calendar, NeedsPermission), (Contacts, Off)],
    ));
    let calendar = switch(&page, Calendar);
    assert!(!calendar.on, "it is not working, so it is not drawn on");
    assert!(calendar.asks_provider);
    assert_eq!(
        calendar.control, IDLE,
        "an only use waiting on permission can still be asked for"
    );
    assert_eq!(calendar.note, Some(AccountUseNote::Withheld));
}

#[test]
fn what_switching_on_needs_is_said_before_the_switch_is_pressed() {
    let account = mailbox_only();
    let page = page(&account, None, None, |capability| match capability {
        Calendar => SwitchOn::AsksProvider,
        Contacts => SwitchOn::NeedsServer,
        _ => SwitchOn::Applies,
    });
    let calendar = switch(&page, Calendar);
    assert!(
        calendar.asks_provider,
        "a grant that lacks it goes to the provider"
    );
    assert_eq!((calendar.control, calendar.note), (IDLE, None));
    let contacts = switch(&page, Contacts);
    assert!(!contacts.asks_provider);
    assert_eq!(
        (contacts.control, contacts.note),
        (HELD, Some(AccountUseNote::NeedsServer)),
        "a use with no server says so rather than failing when pressed"
    );
}

#[test]
fn a_use_the_server_does_not_offer_cannot_be_switched_and_holds_nothing_on() {
    let page = idle(&entry(
        AccountKind::Jmap,
        &[(Mail, On), (Calendar, NotOffered), (Contacts, Off)],
    ));
    let calendar = switch(&page, Calendar);
    assert_eq!(
        (calendar.on, calendar.control, calendar.note),
        (false, HELD, Some(AccountUseNote::NotOffered))
    );
    assert_eq!(
        switch(&page, Mail).note,
        Some(AccountUseNote::Last),
        "mail is the one use that works"
    );
}

#[test]
fn a_use_that_did_not_connect_says_why_and_one_review_follows_the_paused_uses() {
    let mut account = entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, On), (Contacts, On)],
    );
    account.uses[1].problem = Some(UseProblem::ServerChanged);
    account.uses[2].problem = Some(UseProblem::ServerChanged);
    let page_now = idle(&account);
    assert_eq!(
        switch(&page_now, Calendar).note,
        Some(AccountUseNote::ServerChangedCalendar)
    );
    assert_eq!(
        switch(&page_now, Contacts).note,
        Some(AccountUseNote::ServerChangedContacts)
    );
    assert!(!switch(&page_now, Calendar).reviews_server_change);
    assert!(
        switch(&page_now, Contacts).reviews_server_change,
        "one Review, beneath the last of the notes it answers"
    );

    account.uses[1].problem = Some(UseProblem::Unreachable);
    account.uses[2].problem = Some(UseProblem::NeedsOwnSignIn);
    let page_now = idle(&account);
    assert_eq!(
        switch(&page_now, Calendar).note,
        Some(AccountUseNote::UnreachableCalendar)
    );
    assert_eq!(
        switch(&page_now, Contacts).note,
        Some(AccountUseNote::OwnSignInContacts)
    );
    assert!(
        page_now
            .uses
            .iter()
            .all(|switch| !switch.reviews_server_change)
    );
}

#[test]
fn a_picker_is_offered_only_for_a_slot_the_account_can_hold() {
    let mut account = mailbox_only();
    assert_eq!(idle(&account).links, []);

    account.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    account.links.calendar = Some(linked("b@cloud.example"));
    let links = idle(&account).links;
    assert_eq!(links.len(), 1);
    assert_eq!(
        (links[0].slot, links[0].options.len(), links[0].selected),
        (LinkSlot::Calendar, 2, Some(1))
    );
    assert_eq!(links[0].control, IDLE);
}

#[test]
fn a_linked_account_missing_from_the_candidates_is_still_shown() {
    let mut account = mailbox_only();
    account.links.contacts = Some(linked("book@cloud.example"));
    let links = idle(&account).links;
    assert_eq!(links[0].slot, LinkSlot::Contacts);
    assert_eq!(links[0].options, [linked("book@cloud.example")]);
    assert_eq!(links[0].selected, Some(0));
}

#[test]
fn a_suggestion_is_offered_only_where_nothing_is_linked() {
    let mut account = mailbox_only();
    account.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    account.link_candidates.suggested = vec!["b@cloud.example".to_owned()];
    assert_eq!(idle(&account).links[0].suggested, Some(1));

    account.links.calendar = Some(linked("a@cloud.example"));
    assert_eq!(
        idle(&account).links[0].suggested,
        None,
        "a link already made is not second-guessed"
    );
}

#[test]
fn work_running_on_the_page_holds_every_control_and_marks_its_own() {
    let mut account = entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, On), (Contacts, Off)],
    );
    account.link_candidates.calendar = vec![linked("cal@cloud.example")];

    let switching = page(&account, Some(PageWork::Use(Calendar)), None, |_| {
        SwitchOn::Applies
    });
    assert_eq!(
        switch(&switching, Calendar).control,
        ControlState {
            enabled: false,
            running: true,
        }
    );
    assert_eq!(switch(&switching, Contacts).control, HELD);
    assert_eq!(switching.links[0].control, HELD);
    assert_eq!(switching.remove, HELD);

    let linking = page(
        &account,
        Some(PageWork::Link(LinkSlot::Calendar)),
        None,
        |_| SwitchOn::Applies,
    );
    assert!(linking.links[0].control.running && !linking.links[0].control.enabled);
    assert!(
        linking
            .uses
            .iter()
            .all(|switch| !switch.control.enabled && !switch.control.running)
    );

    let removing = page(&account, Some(PageWork::Remove), None, |_| {
        SwitchOn::Applies
    });
    assert!(removing.remove.running && !removing.remove.enabled);

    assert_eq!(idle(&account).remove, IDLE);
}
