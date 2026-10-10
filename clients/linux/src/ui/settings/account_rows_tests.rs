//! What the accounts list and an account's page say, from a snapshot entry.

use mailcal_bindings::{
    AccountCapability, AccountEntry, AccountKind, AccountLinksView, AccountUse, CapabilityState,
    LinkCandidates, LinkSlot, LinkedAccount,
};

use super::{
    UseSwitch, link_pickers, needs_permission, signin_description, signs_in_at_provider, summary,
    use_switch,
};
use crate::l10n;

fn linked(id: &str) -> LinkedAccount {
    LinkedAccount {
        id: id.to_owned(),
        address: id.to_owned(),
    }
}

fn entry(kind: AccountKind, uses: &[(AccountCapability, CapabilityState)]) -> AccountEntry {
    AccountEntry {
        id: "alice@example.org@imap.example.org".to_owned(),
        address: "alice@example.org".to_owned(),
        kind,
        uses: uses
            .iter()
            .map(|(capability, state)| AccountUse {
                capability: *capability,
                state: *state,
            })
            .collect(),
        links: AccountLinksView::default(),
        linked_from: Vec::new(),
        link_candidates: LinkCandidates::default(),
        endpoints: None,
    }
}

fn mailbox_only() -> AccountEntry {
    entry(
        AccountKind::Imap,
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (AccountCapability::Calendar, CapabilityState::Off),
            (AccountCapability::Contacts, CapabilityState::Off),
        ],
    )
}

#[test]
fn the_last_use_cannot_be_switched_off() {
    let entry = mailbox_only();
    assert_eq!(
        use_switch(&entry, AccountCapability::Mail),
        UseSwitch {
            active: true,
            asks: false,
            sensitive: false,
            note: Some(l10n::settings_account_use_last()),
        }
    );
    assert_eq!(
        use_switch(&entry, AccountCapability::Calendar),
        UseSwitch {
            active: false,
            asks: false,
            sensitive: true,
            note: None,
        }
    );
}

#[test]
fn colleagues_wait_for_contacts() {
    let mut entry = entry(
        AccountKind::Microsoft,
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (AccountCapability::Calendar, CapabilityState::On),
            (AccountCapability::Contacts, CapabilityState::Off),
            (AccountCapability::Colleagues, CapabilityState::Off),
        ],
    );
    let colleagues = use_switch(&entry, AccountCapability::Colleagues);
    assert!(!colleagues.sensitive);
    assert_eq!(
        colleagues.note,
        Some(l10n::settings_account_colleagues_needs_contacts())
    );

    entry.uses[2].state = CapabilityState::On;
    assert!(use_switch(&entry, AccountCapability::Colleagues).sensitive);
}

#[test]
fn a_use_waiting_on_permission_is_drawn_off_and_switching_it_on_asks() {
    let entry = entry(
        AccountKind::Google,
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (
                AccountCapability::Calendar,
                CapabilityState::NeedsPermission,
            ),
            (AccountCapability::Contacts, CapabilityState::Off),
            (AccountCapability::Colleagues, CapabilityState::Off),
        ],
    );
    let calendar = use_switch(&entry, AccountCapability::Calendar);
    assert!(!calendar.active, "it is not working, so it is not drawn on");
    assert!(calendar.asks && calendar.sensitive);
    assert_eq!(calendar.note, Some(l10n::settings_account_use_withheld()));
    assert!(needs_permission(&entry));
    assert!(summary(&entry).contains(l10n::settings_account_needs_permission()));
}

#[test]
fn an_only_use_waiting_on_permission_can_still_be_asked_for() {
    let entry = entry(
        AccountKind::Google,
        &[
            (AccountCapability::Mail, CapabilityState::Off),
            (
                AccountCapability::Calendar,
                CapabilityState::NeedsPermission,
            ),
            (AccountCapability::Contacts, CapabilityState::Off),
        ],
    );
    assert_eq!(
        use_switch(&entry, AccountCapability::Calendar),
        UseSwitch {
            active: false,
            asks: true,
            sensitive: true,
            note: Some(l10n::settings_account_use_withheld()),
        }
    );
}

#[test]
fn a_use_the_server_does_not_offer_cannot_be_switched_and_holds_nothing_on() {
    let entry = entry(
        AccountKind::Jmap,
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (AccountCapability::Calendar, CapabilityState::NotOffered),
            (AccountCapability::Contacts, CapabilityState::Off),
        ],
    );
    assert_eq!(
        use_switch(&entry, AccountCapability::Calendar),
        UseSwitch {
            active: false,
            asks: false,
            sensitive: false,
            note: Some(l10n::settings_account_use_not_offered()),
        }
    );
    // Mail is the one use that works, so it is the last.
    assert_eq!(
        use_switch(&entry, AccountCapability::Mail).note,
        Some(l10n::settings_account_use_last())
    );
    assert!(!summary(&entry).contains(l10n::nav_calendar()));
}

#[test]
fn signing_in_again_is_offered_where_the_provider_signs_in_and_says_what_is_wrong() {
    assert!(signs_in_at_provider(AccountKind::Microsoft));
    assert!(signs_in_at_provider(AccountKind::Google));
    for kind in [AccountKind::Imap, AccountKind::Dav, AccountKind::Jmap] {
        assert!(!signs_in_at_provider(kind), "{kind:?}");
    }

    let mut entry = entry(
        AccountKind::Google,
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (AccountCapability::Calendar, CapabilityState::On),
        ],
    );
    assert_eq!(
        signin_description(&entry, false),
        l10n::settings_account_signin_description()
    );
    entry.uses[1].state = CapabilityState::NeedsPermission;
    assert_eq!(
        signin_description(&entry, false),
        l10n::settings_account_needs_permission()
    );
    assert_eq!(
        signin_description(&entry, true),
        l10n::signin_expired_prompt("alice@example.org")
    );
}

#[test]
fn the_summary_names_the_kind_the_uses_and_the_links() {
    let mut entry = mailbox_only();
    entry.links.calendar = Some(linked("alice@cloud.example"));
    let summary = summary(&entry);
    let lines: Vec<_> = summary.lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "{} · {}",
            l10n::setup_account_type_password(),
            l10n::nav_mail()
        )
    );
    assert_eq!(
        lines[1],
        l10n::settings_account_linked_line(l10n::nav_calendar(), "alice@cloud.example")
    );
}

#[test]
fn a_picker_is_offered_only_for_a_slot_the_account_can_hold() {
    let mut entry = mailbox_only();
    assert!(link_pickers(&entry).is_empty());

    entry.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    entry.links.calendar = Some(linked("b@cloud.example"));
    let pickers = link_pickers(&entry);
    assert_eq!(pickers.len(), 1);
    assert_eq!(pickers[0].slot, LinkSlot::Calendar);
    assert_eq!(pickers[0].options.len(), 2);
    assert_eq!(pickers[0].selected, Some(1));
}

#[test]
fn a_linked_account_missing_from_the_candidates_is_still_shown() {
    let mut entry = mailbox_only();
    entry.links.contacts = Some(linked("book@cloud.example"));
    let pickers = link_pickers(&entry);
    assert_eq!(pickers[0].slot, LinkSlot::Contacts);
    assert_eq!(pickers[0].options, [linked("book@cloud.example")]);
    assert_eq!(pickers[0].selected, Some(0));
}

#[test]
fn a_suggestion_is_offered_only_where_nothing_is_linked() {
    let mut entry = mailbox_only();
    entry.link_candidates.calendar = vec![linked("a@cloud.example"), linked("b@cloud.example")];
    entry.link_candidates.suggested = vec!["b@cloud.example".to_owned()];
    assert_eq!(link_pickers(&entry)[0].suggested, Some(1));

    entry.links.calendar = Some(linked("a@cloud.example"));
    assert_eq!(
        link_pickers(&entry)[0].suggested,
        None,
        "a link already made is not second-guessed"
    );
}
