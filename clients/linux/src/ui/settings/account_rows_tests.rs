//! What the accounts list and an account's page say, from a snapshot entry.

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountChangeRefusal, AccountEntry, AccountKind,
    AccountLinksView, AccountUse, AccountUseBlock, AccountUseStatus, AccountUseSwitch,
    CapabilityState, ControlState, LinkCandidates, LinkedAccount, UseProblem,
};

use super::{change_notice, summary, switch_note};
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
        signs_in_separately: false,
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
        refusal: AccountChangeRefusal::Blocked {
            block: AccountUseBlock::LastUse,
        },
    });
    assert_eq!(
        failed,
        Notice::Error {
            title: l10n::settings_account_change_failed_title().to_owned(),
            detail: l10n::settings_account_use_last().to_owned(),
        },
        "in the person's language, as the switch says it beside itself"
    );
    let servers = change_notice(&AccountChangeNotice::ServersFailed {
        refusal: AccountChangeRefusal::SignInRefused,
    });
    assert_eq!(
        servers,
        Notice::Error {
            title: l10n::settings_account_servers_failed().to_owned(),
            detail: l10n::signin_refused().to_owned(),
        }
    );
    let other = change_notice(&AccountChangeNotice::LinkFailed {
        slot: mailcal_bindings::LinkSlot::Calendar,
        refusal: AccountChangeRefusal::Other {
            detail: "config: no such account".to_owned(),
        },
    });
    assert!(matches!(other, Notice::Error { detail, .. } if detail == "config: no such account"));
    let kept = change_notice(&AccountChangeNotice::RemoveFailed {
        detail: "locked".to_owned(),
    });
    assert!(matches!(
        kept,
        Notice::Error { detail, .. } if detail == l10n::remove_account_failed("locked")
    ));
}
