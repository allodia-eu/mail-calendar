// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What the section keeps between signals: one read at a time, a write held until the read after
//! it lands, and nothing that lands for an account that has left.

use super::{Answer, SubscriptionWork, Write};
use crate::{AllodiaPlan, AllodiaSubscriptionOutcome, AllodiaWriteFailure};

const ME: &str = "person@example.test";

/// One read at a time: one asked for while another runs runs once after it, however many asked,
/// since the read out may predate what prompted the ask.
#[test]
fn one_read_at_a_time_and_one_more_after_it() {
    let work = SubscriptionWork::default();
    assert!(!work.asked(ME));
    let ticket = work.begin_read(ME).expect("nothing runs");
    assert!(work.begin_read(ME).is_none());
    assert!(work.begin_read(ME).is_none());
    assert!(work.snapshot(ME).reading);
    let again = work
        .read(ME, ticket, Answer::Unavailable)
        .expect("one more, for the asks meanwhile");
    let held = work.snapshot(ME);
    assert!(held.reading);
    assert_eq!(held.answer, Some(Answer::Unavailable));
    assert_eq!(work.read(ME, again, Answer::NeedsReauth), None);
    assert!(!work.snapshot(ME).reading);
    assert!(work.asked(ME));
    assert!(work.begin_read(ME).is_some());
}

/// A write runs from the press until the read after it lands, refuses a second write, and drops
/// the last outcome when it starts.
#[test]
fn a_write_runs_until_the_read_after_it() {
    let work = SubscriptionWork::default();
    let said = AllodiaSubscriptionOutcome::Failed {
        failure: AllodiaWriteFailure::Unexplained,
    };
    assert!(work.begin_write(ME, Write::Resubscribe));
    work.wrote(ME, Some(said.clone()), None);
    work.write_ended(ME);
    assert_eq!(work.snapshot(ME).outcome, Some(said.clone()));
    assert!(work.begin_write(ME, Write::Cancel));
    assert_eq!(work.snapshot(ME).outcome, None);
    assert!(!work.begin_write(ME, Write::Resubscribe));
    // A refresh while the write runs is answered by the read that follows it.
    assert!(work.begin_read(ME).is_none());

    work.wrote(ME, Some(said.clone()), None);
    let ticket = work.begin_reread(ME);
    assert_eq!(work.snapshot(ME).writing, Some(Write::Cancel));
    work.read(ME, ticket, Answer::NeedsReauth);
    work.write_ended(ME);
    let held = work.snapshot(ME);
    assert_eq!(held.writing, None);
    assert_eq!(held.outcome, Some(said));
    assert_eq!(held.answer, Some(Answer::NeedsReauth));
}

/// ⚠️ A read that started before a write and lands after the read that followed it is older than
/// what the section holds, and is dropped: it would draw the subscription as it was before the
/// write.
#[test]
fn an_older_read_landing_late_is_dropped() {
    let work = SubscriptionWork::default();
    let before = work.begin_read(ME).expect("nothing runs");
    assert!(work.begin_write(ME, Write::Switch(AllodiaPlan::Yearly)));
    let after = work.begin_reread(ME);
    work.read(ME, after, Answer::NeedsReauth);
    work.read(ME, before, Answer::Unavailable);
    let held = work.snapshot(ME);
    assert_eq!(held.answer, Some(Answer::NeedsReauth));
    assert!(!held.reading);
}

/// Each payment page is opened once: its id rises with every page, across accounts too.
#[test]
fn every_payment_page_has_a_new_id() {
    let work = SubscriptionWork::default();
    assert!(work.begin_write(ME, Write::Checkout(AllodiaPlan::Monthly)));
    work.wrote(ME, None, Some("https://pay.example.test/1".to_owned()));
    let first = work.snapshot(ME).page.expect("a page");
    work.forget();
    let other = "other@example.test";
    work.snapshot(other);
    work.wrote(other, None, Some("https://pay.example.test/2".to_owned()));
    let second = work.snapshot(other).page.expect("a page");
    assert!(second.id > first.id);
}

/// ⚠️ What lands for an account that has left is dropped, and the next account starts with
/// nothing: one person's subscription is never drawn under another's sign-in.
#[test]
fn nothing_lands_for_an_account_that_has_left() {
    let work = SubscriptionWork::default();
    let ticket = work.begin_read(ME).expect("nothing runs");
    assert!(work.begin_write(ME, Write::Cancel));
    let other = "other@example.test";
    assert_eq!(work.snapshot(other).answer, None);
    work.read(ME, ticket, Answer::Unavailable);
    work.wrote(
        ME,
        Some(AllodiaSubscriptionOutcome::Restarted),
        Some("https://pay.example.test".to_owned()),
    );
    work.write_ended(ME);
    let held = work.snapshot(other);
    assert_eq!(held.answer, None);
    assert_eq!(held.outcome, None);
    assert_eq!(held.page, None);
    assert_eq!(held.writing, None);
    assert!(!held.reading);

    // Signing out forgets the account's section outright.
    work.read(
        other,
        work.begin_read(other).expect("free"),
        Answer::Unavailable,
    );
    work.forget();
    assert_eq!(work.snapshot(other).answer, None);
}
