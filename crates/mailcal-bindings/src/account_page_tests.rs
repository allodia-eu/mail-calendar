//! An account's page as it is put together: what pressing a switch would come to drawn as the
//! switch, what is wrong with a use said beside it, the link pickers, and how a running change
//! holds the controls. Whether a change is refused is the registry's `toggle`, whose rules are
//! pinned in `account_registry/toggle_tests.rs`; here it is stood in for.

use AccountCapability::{Calendar, Contacts, Mail, Meetings};
use CapabilityState::{NeedsPermission, NotOffered, Off, On};
use ReplyFallbackChoice::{Always, Ask, Never};
use mailcal_account::Capabilities;

use crate::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountPage, AccountUse,
    AccountUseBlock, AccountUseStatus, AccountUseSwitch, CapabilityState, ControlState,
    LinkCandidates, LinkSlot, LinkedAccount, ReplyFallbackChoice, SignInMethod, UseProblem,
    account_registry::Toggle,
    account_work::{Hold, PageWork},
};

pub(super) const IDLE: ControlState = ControlState {
    enabled: true,
    running: false,
};
const HELD: ControlState = ControlState {
    enabled: false,
    running: false,
};

pub(super) fn linked(id: &str) -> LinkedAccount {
    LinkedAccount {
        id: id.to_owned(),
        address: id.to_owned(),
        distinction: None,
    }
}

pub(super) fn entry(
    kind: AccountKind,
    uses: &[(AccountCapability, CapabilityState)],
) -> AccountEntry {
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
        distinction: None,
        mail_sync: None,
        nextcloud: false,
    }
}

pub(super) fn mailbox_only() -> AccountEntry {
    entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, Off), (Contacts, Off)],
    )
}

/// The hold of a page whose own change is `work`.
fn own(work: PageWork) -> Hold {
    Hold {
        busy: true,
        own: vec![work],
    }
}

/// The page of an account nothing takes a use from.
pub(super) fn page(
    entry: &AccountEntry,
    hold: Hold,
    notice: Option<crate::AccountChangeNoticeRaised>,
    stored: Option<crate::AccountEndpoints>,
    refused: Option<crate::AccountEndpoints>,
    fallback: ReplyFallbackChoice,
    toggle: impl Fn(AccountCapability, bool) -> Toggle,
) -> AccountPage {
    super::page(
        entry,
        hold,
        notice,
        Vec::new(),
        stored,
        refused,
        fallback,
        toggle,
    )
}

/// A registry that makes every change.
pub(super) fn applies(_: AccountCapability, _: bool) -> Toggle {
    Toggle::Applies {
        next: Capabilities::default(),
    }
}

pub(super) fn idle(entry: &AccountEntry) -> AccountPage {
    page(entry, Hold::default(), None, None, None, Ask, applies)
}

pub(super) fn switch(page: &AccountPage, capability: AccountCapability) -> &AccountUseSwitch {
    page.uses
        .iter()
        .find(|switch| switch.capability == capability)
        .expect("the page lists the use")
}

#[test]
fn a_switch_the_core_would_refuse_is_held_and_says_why_and_one_it_would_make_is_free() {
    let account = mailbox_only();
    let page = page(
        &account,
        Hold::default(),
        None,
        None,
        None,
        Ask,
        |capability, on| match (capability, on) {
            (Mail, false) => Toggle::Refused(AccountUseBlock::LastUse),
            (Contacts, true) => Toggle::Refused(AccountUseBlock::NeedsServer),
            _ => applies(capability, on),
        },
    );
    assert_eq!(
        switch(&page, Mail),
        &AccountUseSwitch {
            capability: Mail,
            on: true,
            control: HELD,
            blocked: Some(AccountUseBlock::LastUse),
            status: None,
            asks_provider: false,
            confirms_off: true,
            reviews_server_change: false,
            signs_in_separately: false,
            linked: None,
        },
        "an on switch is asked what switching it off would do"
    );
    assert_eq!(
        switch(&page, Contacts).blocked,
        Some(AccountUseBlock::NeedsServer)
    );
    assert_eq!(switch(&page, Contacts).control, HELD);
    let calendar = switch(&page, Calendar);
    assert_eq!(
        (calendar.control, calendar.blocked, calendar.status),
        (IDLE, None, None)
    );
}

#[test]
fn a_switch_whose_grant_lacks_it_asks_the_provider_and_one_waiting_on_it_says_so() {
    let account = entry(
        AccountKind::Google,
        &[(Mail, On), (Calendar, NeedsPermission), (Contacts, Off)],
    );
    let page = page(
        &account,
        Hold::default(),
        None,
        None,
        None,
        Ask,
        |capability, on| match (capability, on) {
            (Calendar | Contacts, true) => Toggle::AsksProvider,
            _ => applies(capability, on),
        },
    );
    let calendar = switch(&page, Calendar);
    assert!(
        !calendar.on,
        "a use waiting on the provider is not working, so it is drawn off"
    );
    assert!(calendar.asks_provider && calendar.control.enabled);
    assert_eq!(calendar.status, Some(AccountUseStatus::Withheld));
    let contacts = switch(&page, Contacts);
    assert!(
        contacts.asks_provider,
        "a grant that lacks it goes to the provider"
    );
    assert_eq!(
        (contacts.status, contacts.blocked),
        (None, None),
        "not chosen, so nothing is wrong"
    );
}

#[test]
fn what_is_wrong_with_a_use_is_said_beside_why_it_cannot_be_pressed() {
    // A calendar-only account whose calendar is unreachable: the switch is held because it is the
    // last use, and the page says both, rather than one hiding the other.
    let mut account = entry(AccountKind::Dav, &[(Calendar, On), (Contacts, Off)]);
    account.uses[0].problem = Some(UseProblem::Unreachable);
    let page = page(
        &account,
        Hold::default(),
        None,
        None,
        None,
        Ask,
        |capability, on| match (capability, on) {
            (Calendar, false) => Toggle::Refused(AccountUseBlock::LastUse),
            _ => applies(capability, on),
        },
    );
    let calendar = switch(&page, Calendar);
    assert_eq!(calendar.blocked, Some(AccountUseBlock::LastUse));
    assert_eq!(
        calendar.status,
        Some(AccountUseStatus::Problem {
            problem: UseProblem::Unreachable
        })
    );
}

#[test]
fn a_use_the_server_does_not_offer_is_asked_about_switching_on() {
    let account = entry(AccountKind::Jmap, &[(Mail, On), (Calendar, NotOffered)]);
    let page = page(
        &account,
        Hold::default(),
        None,
        None,
        None,
        Ask,
        |capability, on| match (capability, on) {
            (Calendar, true) => Toggle::Refused(AccountUseBlock::NotOffered),
            _ => applies(capability, on),
        },
    );
    let calendar = switch(&page, Calendar);
    assert_eq!((calendar.on, calendar.control), (false, HELD));
    assert_eq!(calendar.blocked, Some(AccountUseBlock::NotOffered));
}

#[test]
fn video_meetings_switch_off_without_asking() {
    let page = idle(&entry(AccountKind::Dav, &[(Calendar, On), (Meetings, On)]));
    assert!(!switch(&page, Meetings).confirms_off);
    assert!(switch(&page, Calendar).confirms_off);
}

#[test]
fn one_review_follows_the_uses_paused_for_a_new_server() {
    let mut account = entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, On), (Contacts, On)],
    );
    account.uses[1].problem = Some(UseProblem::ServerChanged);
    account.uses[2].problem = Some(UseProblem::ServerChanged);
    let page_now = idle(&account);
    assert!(!switch(&page_now, Calendar).reviews_server_change);
    assert!(
        switch(&page_now, Contacts).reviews_server_change,
        "one Review, beneath the last of the uses it answers"
    );

    account.uses[2].problem = Some(UseProblem::NeedsOwnSignIn);
    let page_now = idle(&account);
    assert!(switch(&page_now, Calendar).reviews_server_change);
    assert_eq!(
        switch(&page_now, Contacts).status,
        Some(AccountUseStatus::Problem {
            problem: UseProblem::NeedsOwnSignIn
        })
    );
}

#[test]
fn setup_offers_only_the_slots_nothing_fills() {
    let mut account = mailbox_only();
    account.link_candidates.calendar = vec![linked("cal@cloud.example")];
    account.link_candidates.contacts = vec![linked("book@cloud.example")];
    account.links.contacts = Some(linked("book@cloud.example"));
    let offered = super::setup_link_pickers(&account);
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].slot, LinkSlot::Calendar);
}

#[test]
fn work_running_on_the_page_holds_every_control_and_marks_its_own() {
    let mut account = entry(
        AccountKind::Imap,
        &[(Mail, On), (Calendar, On), (Contacts, Off)],
    );
    account.link_candidates.calendar = vec![linked("cal@cloud.example")];

    let switching = page(
        &account,
        own(PageWork::Use {
            capability: Calendar,
            on: false,
        }),
        None,
        None,
        None,
        Ask,
        applies,
    );
    assert_eq!(
        switch(&switching, Calendar).control,
        ControlState {
            enabled: false,
            running: true,
        }
    );
    assert!(
        !switch(&switching, Calendar).on,
        "a running switch is drawn where it is going"
    );
    assert_eq!(switch(&switching, Contacts).control, HELD);
    assert_eq!(switching.links[0].control, HELD);
    assert_eq!(switching.remove, HELD);

    let linking = page(
        &account,
        own(PageWork::Link {
            slot: LinkSlot::Calendar,
            target: Some("cal@cloud.example".to_owned()),
        }),
        None,
        None,
        None,
        Ask,
        applies,
    );
    assert!(linking.links[0].control.running && !linking.links[0].control.enabled);
    assert_eq!(
        linking.links[0].selected,
        Some(0),
        "a running picker shows what it is linking"
    );
    assert!(
        linking
            .uses
            .iter()
            .all(|switch| !switch.control.enabled && !switch.control.running)
    );

    let removing = page(
        &account,
        own(PageWork::Remove),
        None,
        None,
        None,
        Ask,
        applies,
    );
    assert!(removing.remove.running && !removing.remove.enabled);

    // A change running on another account holds this page too, and marks nothing here running:
    // a link is stored on both ends, so two changes at once could race on one account.
    let elsewhere = page(
        &account,
        Hold {
            busy: true,
            own: Vec::new(),
        },
        None,
        None,
        None,
        Ask,
        applies,
    );
    assert!(elsewhere.uses.iter().all(|switch| switch.control == HELD));
    assert_eq!((elsewhere.links[0].control, elsewhere.remove), (HELD, HELD));

    assert_eq!(idle(&account).remove, IDLE);
}

#[test]
fn an_account_used_for_its_calendar_says_what_happens_to_a_reply_its_server_cannot_pass_on() {
    assert_eq!(
        idle(&mailbox_only()).reply_fallback,
        None,
        "no calendar, no choice"
    );

    let calendar = entry(AccountKind::Imap, &[(Mail, On), (Calendar, On)]);
    let chosen = page(&calendar, Hold::default(), None, None, None, Never, applies);
    assert_eq!(
        chosen
            .reply_fallback
            .map(|picker| (picker.choice, picker.control)),
        Some((Never, IDLE))
    );

    let changing = page(
        &calendar,
        own(PageWork::ReplyFallback { choice: Always }),
        None,
        None,
        None,
        Ask,
        applies,
    );
    let picker = changing.reply_fallback.expect("a picker");
    assert_eq!(
        picker.choice, Always,
        "a running picker shows where it is going"
    );
    assert!(picker.control.running && !picker.control.enabled);
}
