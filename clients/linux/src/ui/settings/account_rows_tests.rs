//! What the accounts list and an account's page say, from a snapshot entry.

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountEntry, AccountKind, AccountLinksView,
    AccountUse, AccountUseBlock, AccountUseStatus, AccountUseSwitch, CapabilityState, ControlState,
    LinkCandidates, LinkedAccount, UseProblem,
};

use super::{
    change_notice, outage_line, signin_description, signs_in_at_provider, summary, switch_note,
};
use crate::{l10n, ui::settings::notice::Notice};

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
        sign_in: mailcal_bindings::SignInMethod::Password,
        uses: uses
            .iter()
            .map(|(capability, state)| AccountUse {
                capability: *capability,
                state: *state,
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
        &[
            (AccountCapability::Mail, CapabilityState::On),
            (AccountCapability::Calendar, CapabilityState::Off),
            (AccountCapability::Contacts, CapabilityState::Off),
        ],
    )
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
    entry.distinction = Some(mailcal_bindings::AccountDistinction::Server {
        host: "imap.example.org".to_owned(),
    });
    assert_eq!(
        signin_description(&entry, true),
        l10n::signin_expired_prompt(&l10n::settings_account_distinguished(
            "alice@example.org",
            "imap.example.org"
        ))
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

    let mut waiting = mailbox_only();
    waiting.uses[1].state = CapabilityState::NeedsPermission;
    waiting.uses[2].state = CapabilityState::NotOffered;
    let summary = super::summary(&waiting);
    assert!(summary.starts_with(&format!(
        "{} · {}, {}",
        l10n::setup_account_type_password(),
        l10n::nav_mail(),
        l10n::nav_calendar()
    )));
    assert!(
        !summary.contains(l10n::nav_contacts()),
        "a use the server lacks is not one"
    );
    assert!(summary.ends_with(l10n::settings_account_needs_permission()));
}

#[test]
fn an_outage_line_names_the_account_then_the_reason_its_page_gives_then_the_last_sync() {
    use mailcal_bindings::UseProblem;
    let address = "alice@example.org";
    for (capability, problem, title, reason) in [
        (
            AccountCapability::Calendar,
            UseProblem::NeedsOwnSignIn,
            l10n::calendar_outage_title(address),
            l10n::settings_account_use_own_sign_in_calendar(),
        ),
        (
            AccountCapability::Calendar,
            UseProblem::SignInRefused,
            l10n::calendar_outage_title(address),
            l10n::settings_account_use_refused_calendar(),
        ),
        (
            AccountCapability::Calendar,
            UseProblem::Unreachable,
            l10n::calendar_outage_title(address),
            l10n::settings_account_use_unreachable_calendar(),
        ),
        (
            AccountCapability::Contacts,
            UseProblem::NeedsOwnSignIn,
            l10n::contacts_outage_title(address),
            l10n::settings_account_use_own_sign_in_contacts(),
        ),
        (
            AccountCapability::Contacts,
            UseProblem::SignInRefused,
            l10n::contacts_outage_title(address),
            l10n::settings_account_use_refused_contacts(),
        ),
        (
            AccountCapability::Contacts,
            UseProblem::Unreachable,
            l10n::contacts_outage_title(address),
            l10n::settings_account_use_unreachable_contacts(),
        ),
        (
            AccountCapability::Calendar,
            UseProblem::ServerChanged,
            l10n::calendar_outage_title(address),
            l10n::settings_account_use_server_changed_calendar(),
        ),
        (
            AccountCapability::Contacts,
            UseProblem::ServerChanged,
            l10n::contacts_outage_title(address),
            l10n::settings_account_use_server_changed_contacts(),
        ),
    ] {
        assert!(title.contains(address));
        assert_eq!(
            outage_line(capability, problem, address),
            format!("{title} {reason} {}", l10n::use_outage_last_synced()),
            "{capability:?} {problem:?}"
        );
    }
}

#[test]
fn a_switch_says_what_is_wrong_with_the_use_then_why_it_cannot_be_pressed() {
    let switch = |status, blocked: Option<AccountUseBlock>| AccountUseSwitch {
        capability: AccountCapability::Calendar,
        on: true,
        control: ControlState {
            enabled: blocked.is_none(),
            running: false,
        },
        blocked,
        status,
        asks_provider: false,
        confirms_off: true,
        reviews_server_change: false,
    };
    assert_eq!(switch_note(&switch(None, None)), None);
    assert_eq!(
        switch_note(&switch(None, Some(AccountUseBlock::LastUse))).as_deref(),
        Some(l10n::settings_account_use_last())
    );
    assert_eq!(
        switch_note(&switch(Some(AccountUseStatus::Withheld), None)).as_deref(),
        Some(l10n::settings_account_use_withheld())
    );
    let unreachable = AccountUseStatus::Problem {
        problem: UseProblem::Unreachable,
    };
    assert_eq!(
        switch_note(&switch(Some(unreachable), Some(AccountUseBlock::LastUse))),
        Some(format!(
            "{}\n{}",
            l10n::settings_account_use_unreachable_calendar(),
            l10n::settings_account_use_last()
        )),
        "neither hides the other"
    );
}

#[test]
fn a_change_that_did_not_take_is_said_with_the_cores_reason() {
    let failed = change_notice(&AccountChangeNotice::UseFailed {
        capability: AccountCapability::Mail,
        on: false,
        detail: "the last use".to_owned(),
    });
    assert_eq!(
        failed,
        Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: "the last use".to_owned(),
        }
    );
    let kept = change_notice(&AccountChangeNotice::RemoveFailed {
        detail: "locked".to_owned(),
    });
    assert!(matches!(
        kept,
        Notice::Error { detail, .. } if detail == l10n::remove_account_failed("locked")
    ));
}
