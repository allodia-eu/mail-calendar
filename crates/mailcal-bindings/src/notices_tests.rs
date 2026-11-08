//! The notices contract (`docs/notices.md`): what each fact becomes, in what order, one per
//! account and fact, and what dismissing and a failed remedy do.

use super::{Dismissal, NoticeBook, NoticeFacts, Questions, raise};
use crate::{
    AccountProvider, ConnectivitySnapshot, FolderAction, FolderNotice, FolderProblem, Notice,
    NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity, UseOutage, UseProblem,
};

fn quiet() -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        offline: false,
        unreachable_accounts: Vec::new(),
        calendar_reauth_accounts: Vec::new(),
        mail_reauth_accounts: Vec::new(),
        signin_expired_accounts: Vec::new(),
        calendar_outages: Vec::new(),
        contacts_outages: Vec::new(),
        allodia_sign_in_ended: None,
        allodia_lapses_soon: false,
    }
}

fn ids(accounts: &[&str]) -> Vec<String> {
    accounts.iter().map(|id| (*id).to_owned()).collect()
}

fn mail_address(account: &str) -> String {
    format!("{account}@example.org")
}

fn oauth(_: &str) -> Option<AccountProvider> {
    Some(AccountProvider::Google)
}

fn folder_refused(account: &str) -> FolderNotice {
    FolderNotice {
        account: account.to_owned(),
        folder: "Receipts".to_owned(),
        action: FolderAction::Rename,
        problem: FolderProblem::Refused,
    }
}

/// The notices `book` ranks from these facts, with every account an OAuth one.
fn pull(book: &mut NoticeBook, connectivity: &ConnectivitySnapshot) -> Vec<Notice> {
    pull_with(book, connectivity, 0, None)
}

fn pull_with(
    book: &mut NoticeBook,
    connectivity: &ConnectivitySnapshot,
    skipped_at_launch: usize,
    folder_notice: Option<FolderNotice>,
) -> Vec<Notice> {
    let facts = NoticeFacts {
        connectivity,
        skipped_at_launch,
        folder_notice,
        provider: &oauth,
        address: &mail_address,
        questions: &Questions::default(),
    };
    book.snapshot(raise(&facts)).notices
}

fn only(notices: &[Notice]) -> &Notice {
    assert_eq!(notices.len(), 1, "one notice expected: {notices:?}");
    &notices[0]
}

#[test]
fn nothing_wrong_is_no_notice() {
    assert!(pull(&mut NoticeBook::default(), &quiet()).is_empty());
}

#[test]
fn the_launch_skip_and_a_refused_folder_change_are_events_that_can_be_closed() {
    let mut book = NoticeBook::default();
    let skipped = pull_with(&mut book, &quiet(), 2, None);
    let skipped = only(&skipped);
    assert_eq!(skipped.kind, NoticeKind::AccountsSkipped { count: 2 });
    assert_eq!(
        (skipped.severity, skipped.screens),
        (NoticeSeverity::Degraded, NoticeScreens::Mail)
    );
    assert_eq!((skipped.account.as_deref(), skipped.action), (None, None));
    assert_eq!(skipped.mark, None);
    assert!(skipped.dismissible);

    let refused = pull_with(
        &mut NoticeBook::default(),
        &quiet(),
        0,
        Some(folder_refused("a")),
    );
    let refused = only(&refused);
    assert_eq!(
        refused.kind,
        NoticeKind::FolderChangeRefused {
            folder: "Receipts".to_owned(),
            action: FolderAction::Rename,
            problem: FolderProblem::Refused,
        }
    );
    assert_eq!(
        (refused.severity, refused.screens),
        (NoticeSeverity::Degraded, NoticeScreens::Mail)
    );
    assert_eq!(
        (refused.account.as_deref(), refused.action),
        (Some("a"), None)
    );
    assert_eq!(
        refused.mark.as_ref().map(|words| words.key.as_str()),
        Some("notice_mark_folder_change_refused")
    );
    assert!(refused.dismissible);
}

#[test]
fn a_dead_password_sign_in_opens_settings_and_an_oauth_one_signs_in_again() {
    let connectivity = ConnectivitySnapshot {
        signin_expired_accounts: ids(&["google", "imap", "unknown"]),
        ..quiet()
    };
    let provider = |id: &str| match id {
        "google" => Some(AccountProvider::Google),
        "imap" => Some(AccountProvider::Password),
        _ => None,
    };
    let facts = NoticeFacts {
        connectivity: &connectivity,
        skipped_at_launch: 0,
        folder_notice: None,
        provider: &provider,
        address: &mail_address,
        questions: &Questions::default(),
    };
    let notices = NoticeBook::default().snapshot(raise(&facts)).notices;
    let action = |account: &str| {
        notices
            .iter()
            .find(|notice| notice.account.as_deref() == Some(account))
            .and_then(|notice| notice.action)
    };
    assert_eq!(action("google"), Some(NoticeAction::SignInAgain));
    assert_eq!(action("imap"), Some(NoticeAction::OpenAccountSettings));
    assert_eq!(action("unknown"), Some(NoticeAction::OpenAccountSettings));
}

#[test]
fn three_expired_sign_ins_are_three_notices_each_with_its_own_button() {
    let connectivity = ConnectivitySnapshot {
        signin_expired_accounts: ids(&["c", "a", "b"]),
        ..quiet()
    };
    let notices = pull(&mut NoticeBook::default(), &connectivity);
    let accounts: Vec<_> = notices.iter().map(|n| n.account.as_deref()).collect();
    assert_eq!(accounts, [Some("a"), Some("b"), Some("c")]);
    assert!(
        notices
            .iter()
            .all(|n| n.action == Some(NoticeAction::SignInAgain))
    );
    let mut ids: Vec<_> = notices.iter().map(|n| n.id.as_str()).collect();
    ids.dedup();
    assert_eq!(ids.len(), 3, "each notice has its own id");
}

#[test]
fn two_facts_about_one_account_are_two_notices() {
    let connectivity = ConnectivitySnapshot {
        mail_reauth_accounts: ids(&["a"]),
        calendar_reauth_accounts: ids(&["a"]),
        ..quiet()
    };
    let notices = pull(&mut NoticeBook::default(), &connectivity);
    assert_eq!(notices.len(), 2);
    assert_ne!(notices[0].id, notices[1].id);
}

#[test]
fn rank_is_severity_then_app_before_account_then_age() {
    let mut book = NoticeBook::default();
    // Raised first, so older than everything raised below.
    let first = ConnectivitySnapshot {
        unreachable_accounts: ids(&["z"]),
        ..quiet()
    };
    pull(&mut book, &first);
    let later = ConnectivitySnapshot {
        offline: false,
        unreachable_accounts: ids(&["a", "z"]),
        signin_expired_accounts: ids(&["s"]),
        allodia_sign_in_ended: Some("me@allodia.example".to_owned()),
        ..quiet()
    };
    let ranked: Vec<_> = pull(&mut book, &later).into_iter().map(|n| n.id).collect();
    assert_eq!(
        ranked,
        [
            // Blocked first, whatever its age.
            "signin_expired:s",
            // Degraded: the app before any account.
            "allodia_sign_in_ended",
            // Then older before newer, though "a" sorts before "z".
            "unreachable:z",
            "unreachable:a",
        ]
    );

    let offline = ConnectivitySnapshot {
        offline: true,
        ..later
    };
    let ranked = pull(&mut book, &offline);
    assert_eq!(
        ranked.last().map(|n| n.kind.clone()),
        Some(NoticeKind::Offline),
        "Informational goes last, though it is about the whole app"
    );
}

#[test]
fn a_notice_raised_again_after_it_went_is_newer() {
    let mut book = NoticeBook::default();
    let both = ConnectivitySnapshot {
        unreachable_accounts: ids(&["a", "b"]),
        ..quiet()
    };
    pull(&mut book, &both);
    pull(
        &mut book,
        &ConnectivitySnapshot {
            unreachable_accounts: ids(&["b"]),
            ..quiet()
        },
    );
    let ranked: Vec<_> = pull(&mut book, &both).into_iter().map(|n| n.id).collect();
    assert_eq!(ranked, ["unreachable:b", "unreachable:a"]);
}

#[test]
fn a_dismissed_event_stays_closed_until_its_fact_goes_and_comes_back() {
    let mut book = NoticeBook::default();
    let notices = pull_with(&mut book, &quiet(), 1, None);
    let id = only(&notices).id.clone();

    assert_eq!(book.dismiss(&id), Some(Dismissal::Remembered));
    assert_eq!(book.dismiss(&id), None, "closing it again changes nothing");
    assert!(pull_with(&mut book, &quiet(), 1, None).is_empty());
    assert!(
        pull_with(&mut book, &quiet(), 1, None).is_empty(),
        "it stays closed"
    );

    pull_with(&mut book, &quiet(), 0, None);
    assert_eq!(
        pull_with(&mut book, &quiet(), 1, None).len(),
        1,
        "a new skip is a new notice"
    );
}

#[test]
fn a_state_that_holds_cannot_be_dismissed() {
    let mut book = NoticeBook::default();
    let connectivity = ConnectivitySnapshot {
        signin_expired_accounts: ids(&["a"]),
        ..quiet()
    };
    let notices = pull(&mut book, &connectivity);
    assert_eq!(book.dismiss(&only(&notices).id), None);
    assert_eq!(pull(&mut book, &connectivity).len(), 1);
    assert_eq!(book.dismiss("no-such-notice"), None);
}

#[test]
fn a_refused_folder_change_is_dismissed_where_the_pane_holds_it() {
    let mut book = NoticeBook::default();
    let notices = pull_with(&mut book, &quiet(), 0, Some(folder_refused("a")));
    let id = only(&notices).id.clone();
    assert_eq!(
        book.dismiss(&id),
        Some(Dismissal::FolderNotice(folder_refused("a")))
    );
    // The book hides nothing of its own: a refusal still standing in the pane is still shown.
    assert_eq!(
        pull_with(&mut book, &quiet(), 0, Some(folder_refused("a"))).len(),
        1
    );
}

#[test]
fn a_failed_remedy_is_marked_until_the_notice_goes() {
    let mut book = NoticeBook::default();
    let expired = ConnectivitySnapshot {
        signin_expired_accounts: ids(&["a", "b"]),
        ..quiet()
    };
    pull(&mut book, &expired);
    assert!(book.note_remedy_failed("signin_expired:a"));
    let notices = pull(&mut book, &expired);
    let failed: Vec<_> = notices
        .iter()
        .map(|n| (n.id.as_str(), n.remedy_failed))
        .collect();
    assert_eq!(
        failed,
        [("signin_expired:a", true), ("signin_expired:b", false)]
    );

    pull(&mut book, &quiet());
    let notices = pull(&mut book, &expired);
    assert!(
        notices.iter().all(|n| !n.remedy_failed),
        "a new notice has not failed yet"
    );
}

#[test]
fn a_notice_with_no_action_has_no_remedy_to_fail() {
    let mut book = NoticeBook::default();
    let offline = ConnectivitySnapshot {
        offline: true,
        ..quiet()
    };
    let notices = pull(&mut book, &offline);
    assert!(!book.note_remedy_failed(&only(&notices).id));
    assert!(!book.note_remedy_failed("signin_expired:nobody"));
    assert!(!only(&pull(&mut book, &offline)).remedy_failed);
}

#[test]
fn a_dismiss_aimed_at_a_replaced_folder_refusal_closes_nothing() {
    let mut book = NoticeBook::default();
    let first = pull_with(&mut book, &quiet(), 0, Some(folder_refused("a")));
    let first = only(&first).id.clone();
    let mut later = folder_refused("a");
    later.action = FolderAction::Delete;
    let second = pull_with(&mut book, &quiet(), 0, Some(later));
    assert_ne!(only(&second).id, first, "a later refusal is a new notice");
    assert_eq!(book.dismiss(&first), None);
}

#[test]
fn a_failed_remedy_is_forgotten_when_the_notice_offers_another() {
    let mut book = NoticeBook::default();
    let outage = |problem| ConnectivitySnapshot {
        calendar_outages: vec![UseOutage {
            account_id: "a".to_owned(),
            problem,
        }],
        ..quiet()
    };
    let notices = pull(&mut book, &outage(UseProblem::Unreachable));
    assert!(book.note_remedy_failed(&only(&notices).id));
    assert!(
        !book.note_remedy_failed(&only(&notices).id),
        "reported once"
    );
    let refused = pull(&mut book, &outage(UseProblem::SignInRefused));
    let refused = only(&refused);
    assert_eq!(refused.action, Some(NoticeAction::OpenAccountSettings));
    assert!(!refused.remedy_failed, "nobody has tried the new remedy");
    assert!(!only(&pull(&mut book, &outage(UseProblem::Unreachable))).remedy_failed);
}

#[path = "notices_mapping_tests.rs"]
mod mapping;

#[path = "notices_retry_tests.rs"]
mod retry;

#[test]
fn the_allodia_notice_is_blocked_while_a_paid_capability_is_about_to_lapse() {
    let mut book = NoticeBook::default();
    let ended = ConnectivitySnapshot {
        allodia_sign_in_ended: Some("me@allodia.example".to_owned()),
        unreachable_accounts: ids(&["z"]),
        ..quiet()
    };
    let severity = |notices: &[crate::Notice]| {
        notices
            .iter()
            .find(|notice| notice.id == "allodia_sign_in_ended")
            .map(|notice| notice.severity)
    };
    assert_eq!(
        severity(&pull(&mut book, &ended)),
        Some(NoticeSeverity::Degraded)
    );

    let lapsing = ConnectivitySnapshot {
        allodia_lapses_soon: true,
        ..ended
    };
    let ranked = pull(&mut book, &lapsing);
    assert_eq!(severity(&ranked), Some(NoticeSeverity::Blocked));
    assert_eq!(
        ranked[0].id, "allodia_sign_in_ended",
        "Blocked is drawn first"
    );
}
