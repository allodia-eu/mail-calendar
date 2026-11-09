//! The browser sign-in's steps, as the core keeps them: one out at a time, a fresh one
//! superseding it, a question before the browser, a completion nothing supersedes, and a
//! Nextcloud grant kept for the next try.

use super::{Began, BrowserSignIns, Completion, Ended, Pending, Start};
use crate::{AccountCapability, BrowserSignInStage, UnreachedUses};

fn began(pending: Pending, unreached: Vec<AccountCapability>) -> Began {
    let redirect_uri = match pending {
        Pending::Nextcloud(_) => None,
        _ => Some("app://imap-oauth".to_owned()),
    };
    Began {
        pending,
        url: "https://example.org/authorize".to_owned(),
        redirect_uri,
        unreached,
    }
}

fn begun(sign_ins: &BrowserSignIns, account: &str) -> u64 {
    match sign_ins.start(account, &[], false) {
        Start::Begin { attempt, .. } => attempt,
        other => panic!("expected a sign-in to begin, got {other:?}"),
    }
}

fn stage(sign_ins: &BrowserSignIns) -> Option<BrowserSignInStage> {
    sign_ins.record().map(|record| record.stage)
}

#[test]
fn a_fresh_sign_in_supersedes_the_one_out_and_what_its_server_answers_is_dropped() {
    let sign_ins = BrowserSignIns::default();
    let first = begun(&sign_ins, "a");
    let second = begun(&sign_ins, "a");
    assert!(second > first);
    assert_eq!(
        sign_ins.began(first, began(Pending::Account("one".into()), Vec::new())),
        None
    );
    assert_eq!(stage(&sign_ins), Some(BrowserSignInStage::Starting));
    assert!(
        sign_ins
            .began(second, began(Pending::Account("two".into()), Vec::new()))
            .is_some()
    );
    assert!(!sign_ins.returned(first, "app://imap-oauth?code=1".into()));
    assert!(sign_ins.returned(second, "app://imap-oauth?code=2".into()));
    let run = sign_ins.take(second).expect("the completion");
    assert_eq!(
        run.completion,
        Completion::Account {
            pending: "two".into(),
            callback_url: "app://imap-oauth?code=2".into(),
            unreached: None,
        }
    );
}

#[test]
fn a_switch_that_would_cost_a_use_asks_before_the_browser_and_carries_the_answer() {
    let sign_ins = BrowserSignIns::default();
    let attempt = begun(&sign_ins, "a");
    let calendar = vec![AccountCapability::Calendar];
    sign_ins.began(
        attempt,
        began(Pending::Account("p".into()), calendar.clone()),
    );
    assert_eq!(
        stage(&sign_ins),
        Some(BrowserSignInStage::Asking {
            unreached: calendar
        })
    );
    assert!(
        !sign_ins.returned(attempt, "app://imap-oauth?code=1".into()),
        "nothing returns before the browser opens"
    );
    assert!(
        sign_ins
            .answered(attempt, Some(UnreachedUses::KeepOnPassword))
            .is_some()
    );
    assert!(matches!(
        stage(&sign_ins),
        Some(BrowserSignInStage::InBrowser {
            redirect_uri: Some(_),
            ..
        })
    ));
    sign_ins.returned(attempt, "app://imap-oauth?code=1".into());
    assert!(matches!(
        sign_ins.take(attempt).map(|run| run.completion),
        Some(Completion::Account {
            unreached: Some(UnreachedUses::KeepOnPassword),
            ..
        })
    ));
}

#[test]
fn cancelling_the_question_ends_the_sign_in() {
    let sign_ins = BrowserSignIns::default();
    let attempt = begun(&sign_ins, "a");
    sign_ins.began(
        attempt,
        began(
            Pending::Account("p".into()),
            vec![AccountCapability::Contacts],
        ),
    );
    assert_eq!(sign_ins.answered(attempt, None), None);
    assert_eq!(sign_ins.record(), None);
}

#[test]
fn a_sign_in_being_completed_is_neither_superseded_nor_cancelled() {
    let sign_ins = BrowserSignIns::default();
    let attempt = begun(&sign_ins, "a");
    sign_ins.began(attempt, began(Pending::Consent("p".into()), Vec::new()));
    sign_ins.returned(attempt, "app://auth?code=1".into());
    let record = sign_ins.record().expect("still out");
    assert_eq!(record.stage, BrowserSignInStage::Completing);
    assert!(!record.cancel.enabled);
    assert_eq!(sign_ins.start("a", &[], false), Start::Refused);
    assert_eq!(sign_ins.end(attempt), None);
    sign_ins.take(attempt);
    sign_ins.completed(attempt, None);
    assert_eq!(sign_ins.record(), None);
}

#[test]
fn a_wait_ends_on_its_cap_only_if_nothing_moved_it_since() {
    let sign_ins = BrowserSignIns::default();
    let attempt = begun(&sign_ins, "a");
    let asked = sign_ins
        .began(
            attempt,
            began(
                Pending::Account("p".into()),
                vec![AccountCapability::Calendar],
            ),
        )
        .expect("asking");
    let browsing = sign_ins
        .answered(attempt, Some(UnreachedUses::StopUsing))
        .expect("in the browser");
    assert_eq!(sign_ins.timed_out(attempt, asked), None);
    assert_eq!(
        sign_ins.timed_out(attempt, browsing),
        Some(Ended {
            account: "a".to_owned(),
            switching: false,
        })
    );
}

#[test]
fn a_nextcloud_grant_completes_the_sign_in_and_one_for_a_sign_in_that_is_over_is_refused() {
    let sign_ins = BrowserSignIns::default();
    let first = begun(&sign_ins, "a");
    sign_ins.began(first, began(Pending::Nextcloud("poll".into()), Vec::new()));
    assert_eq!(sign_ins.polling(first).as_deref(), Some("poll"));
    assert!(!sign_ins.redirects(first));
    assert!(
        !sign_ins.returned(first, "app://x".into()),
        "a Nextcloud sign-in returns nothing"
    );
    let second = begun(&sign_ins, "a");
    assert_eq!(sign_ins.polling(first), None);
    assert!(!sign_ins.granted(first, "grant".into()));
    sign_ins.began(second, began(Pending::Nextcloud("poll".into()), Vec::new()));
    assert!(sign_ins.granted(second, "grant".into()));
    assert_eq!(
        sign_ins.take(second).map(|run| run.completion),
        Some(Completion::Nextcloud {
            grant: "grant".into()
        })
    );
}

#[test]
fn a_kept_grant_completes_its_accounts_next_sign_in_and_is_withdrawn_by_anothers() {
    let sign_ins = BrowserSignIns::default();
    let attempt = begun(&sign_ins, "a");
    sign_ins.began(
        attempt,
        began(Pending::Nextcloud("poll".into()), Vec::new()),
    );
    sign_ins.granted(attempt, "grant".into());
    sign_ins.take(attempt);
    sign_ins.completed(attempt, Some("grant".into()));

    let again = match sign_ins.start("a", &[], true) {
        Start::Complete { attempt } => attempt,
        other => panic!("expected the kept grant to complete it, got {other:?}"),
    };
    let run = sign_ins.take(again).expect("the kept grant");
    assert!(run.switching);
    sign_ins.completed(again, Some("grant".into()));

    assert!(matches!(
        sign_ins.start("b", &[], false),
        Start::Begin {
            withdraw: Some(grant),
            ..
        } if grant == "grant"
    ));
}

#[test]
fn the_page_reads_only_its_own_accounts_sign_in_with_what_it_asks_for() {
    let sign_ins = BrowserSignIns::default();
    sign_ins.start("a", &[AccountCapability::Calendar], false);
    assert_eq!(sign_ins.of("b"), None);
    assert_eq!(
        sign_ins.of("a").map(|(_, adding)| adding),
        Some(vec![AccountCapability::Calendar])
    );
    assert_eq!(sign_ins.account().as_deref(), Some("a"));
    assert_eq!(sign_ins.forget("a"), None);
    assert_eq!(sign_ins.record(), None);
}
