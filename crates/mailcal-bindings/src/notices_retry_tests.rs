//! A Retry is the core's to judge: the client cannot tell when the dial it started has ended, so
//! the notice is marked when that dial ends and the notice still stands (`docs/notices.md`
//! rule 10).

use super::{ids, only, pull, quiet};
use crate::{ConnectivitySnapshot, notices::NoticeBook};

fn unreachable(accounts: &[&str]) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        unreachable_accounts: ids(accounts),
        ..quiet()
    }
}

#[test]
fn a_retry_whose_dial_ended_with_the_notice_still_standing_failed() {
    let mut book = NoticeBook::default();
    let notices = pull(&mut book, &unreachable(&["a", "b"]));
    assert!(book.note_retry(&notices[0].id));
    assert!(
        !book.settle_retries().is_empty(),
        "the dial ended and the notice stands"
    );
    let failed: Vec<_> = pull(&mut book, &unreachable(&["a", "b"]))
        .into_iter()
        .map(|n| (n.id, n.remedy_failed))
        .collect();
    assert_eq!(
        failed,
        [
            ("unreachable:a".to_owned(), true),
            ("unreachable:b".to_owned(), false)
        ],
        "only the notice that was retried"
    );
}

#[test]
fn a_retry_whose_notice_went_did_not_fail() {
    let mut book = NoticeBook::default();
    let notices = pull(&mut book, &unreachable(&["a"]));
    assert!(book.note_retry(&only(&notices).id));
    // The dial reached the server, so the next snapshot no longer raises it.
    assert!(pull(&mut book, &quiet()).is_empty());
    assert!(book.settle_retries().is_empty());
    assert!(!only(&pull(&mut book, &unreachable(&["a"]))).remedy_failed);
}

#[test]
fn a_dial_nobody_asked_for_settles_nothing() {
    let mut book = NoticeBook::default();
    pull(&mut book, &unreachable(&["a"]));
    assert!(book.settle_retries().is_empty());
    assert!(!only(&pull(&mut book, &unreachable(&["a"]))).remedy_failed);
}

#[test]
fn only_a_notice_that_offers_retry_can_be_retried() {
    let mut book = NoticeBook::default();
    let expired = ConnectivitySnapshot {
        signin_expired_accounts: ids(&["a"]),
        ..quiet()
    };
    let notices = pull(&mut book, &expired);
    assert!(
        !book.note_retry(&only(&notices).id),
        "its remedy is a sign-in"
    );
    assert!(!book.note_retry("unreachable:nobody"));
}

#[test]
fn a_retry_is_settled_once() {
    let mut book = NoticeBook::default();
    let notices = pull(&mut book, &unreachable(&["a"]));
    book.note_retry(&only(&notices).id);
    assert!(!book.settle_retries().is_empty());
    assert!(
        book.settle_retries().is_empty(),
        "a later dial is nobody's retry"
    );
}
