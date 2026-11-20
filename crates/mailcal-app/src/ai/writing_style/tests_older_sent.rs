//! Learning from sent mail older than the account's sync window (`docs/ai.md` rule 11): the review
//! fetches the Sent mail a range reaches and the device does not hold, reads it, and leaves the
//! device holding what it held before; a fetch that fails reads what the device holds and says so.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_api::{AccountId, CalendarDate, MailboxId, MailboxWindows, SyncWindow};
use time::{Date, Duration};

use super::{
    LearnRange, LearningIntent, LearningStage, RangeFetch, Review, SentReach,
    range::today,
    tests_support::{Learner, REPLY, sent},
};
use crate::tests_fakes::{FakeProvider, message};

/// A message in the Sent folder, sent `days` ago.
fn sent_days_ago(id: &str, days: i64) -> engine_api::Message {
    let at = time::OffsetDateTime::now_utc() - Duration::days(days);
    let mut sent = message(id, "sent", "Re: Planning");
    sent.sent_at = Some(
        engine_api::UtcDateTime::new(at.year(), u8::from(at.month()), at.day(), 9, 0, 0).unwrap(),
    );
    sent
}

fn account() -> AccountId {
    AccountId::try_from("acct-1").unwrap()
}

fn sent_mailbox() -> MailboxId {
    MailboxId::try_from("sent").unwrap()
}

fn calendar_day(day: Date) -> CalendarDate {
    CalendarDate::new(day.year(), u8::from(day.month()), day.day()).unwrap()
}

/// An app whose Sent folder holds a message from yesterday and one from 400 days ago, behind a
/// provider that fetches only what its windows admit; the windows each pass asked for.
async fn learner() -> (Learner, Arc<Mutex<Vec<MailboxWindows>>>, FakeProviderSwitch) {
    let provider =
        FakeProvider::with_sent_and_archive(vec![sent("s1"), sent_days_ago("s-old", 400)])
            .with_source(REPLY.as_bytes());
    let fail = provider.failure_switch();
    let (provider, windows) = provider.windowed();
    let learner = Learner::over(vec![("acct-1", provider)]).await;
    (learner, windows, fail)
}

type FakeProviderSwitch = Arc<std::sync::atomic::AtomicBool>;

async fn holds(learner: &Learner, key: &str) -> bool {
    let messages = learner.app.engine.messages(&account()).await.unwrap();
    messages.iter().any(|message| message.id.as_str() == key)
}

/// The review once the read has settled, and whether it says the older mail was not fetched.
fn reviewed(learner: &Learner) -> (Review, bool) {
    match learner.view().stage {
        LearningStage::Review {
            state,
            reach,
            older_not_fetched,
            ..
        } => {
            assert_eq!(
                reach,
                SentReach::of(mailcal_account::SyncDepth::default(), today()),
                "what the device holds, which the fetch leaves as it was"
            );
            (state, older_not_fetched)
        }
        other => panic!("not on Review: {other:?}"),
    }
}

async fn review_range(learner: &Learner, range: LearnRange) {
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::SelectRange { range }).await;
    learner.press(LearningIntent::Next).await;
}

#[tokio::test]
async fn a_range_before_the_sync_window_fetches_the_older_sent_mail_for_its_read() {
    let (learner, windows, _) = learner().await;
    assert!(!holds(&learner, "s-old").await, "outside the window");
    let since = today() - Duration::days(500);
    review_range(
        &learner,
        LearnRange::Between {
            since,
            until: today(),
        },
    )
    .await;

    let (state, older_not_fetched) = reviewed(&learner);
    let Review::Ready(report) = state else {
        panic!("not ready: {state:?}");
    };
    assert_eq!(report.found, 2, "the older message was read too");
    assert!(!older_not_fetched);
    assert!(
        windows.lock().unwrap().iter().any(|asked| {
            asked.window_for(&sent_mailbox()) == SyncWindow::since(calendar_day(since))
                && asked.account() != asked.window_for(&sent_mailbox())
        }),
        "Sent alone was held further back"
    );

    assert!(!holds(&learner, "s-old").await, "removed once read");
    assert!(holds(&learner, "s1").await);
    let last = windows.lock().unwrap().last().cloned().unwrap();
    assert_eq!(
        last.deepened().count(),
        0,
        "the account syncs under its own window again"
    );
}

/// What the range stage says Next fetches, for each range chosen, on an account holding the
/// default sync depth.
fn fetch_said(learner: &Learner) -> RangeFetch {
    match learner.view().stage {
        LearningStage::ChooseRange { fetch, .. } => fetch,
        other => panic!("not on ChooseRange: {other:?}"),
    }
}

#[tokio::test]
async fn the_range_stage_says_what_next_fetches_before_it_is_pressed() {
    let (learner, windows, _) = learner().await;
    let passes = windows.lock().unwrap().len();
    learner.press(LearningIntent::Open).await;
    assert_eq!(
        fetch_said(&learner),
        RangeFetch::Older,
        "the last twelve months"
    );
    let choose = |range| LearningIntent::SelectRange { range };
    learner.press(choose(LearnRange::Everything)).await;
    assert_eq!(fetch_said(&learner), RangeFetch::WholeHistory);
    learner.press(choose(LearnRange::UntilEndOfLastYear)).await;
    assert_eq!(fetch_said(&learner), RangeFetch::WholeHistory);
    let recent = LearnRange::Between {
        since: today() - Duration::days(10),
        until: today(),
    };
    learner.press(choose(recent)).await;
    assert_eq!(fetch_said(&learner), RangeFetch::Nothing);
    assert_eq!(
        windows.lock().unwrap().len(),
        passes,
        "choosing fetches nothing"
    );
}

/// The default reads a year back: the older part of it is fetched, and mail older than a year
/// is not read.
#[tokio::test]
async fn the_last_twelve_months_fetch_and_read_a_year_back() {
    let (learner, windows, _) = learner().await;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;

    let (state, older_not_fetched) = reviewed(&learner);
    assert!(matches!(state, Review::Ready(report) if report.found == 1));
    assert!(!older_not_fetched);
    let a_year_back = LearnRange::LastTwelveMonths.first_day(today()).unwrap();
    assert!(
        windows
            .lock()
            .unwrap()
            .iter()
            .any(|asked| asked.window_for(&sent_mailbox())
                == SyncWindow::since(calendar_day(a_year_back)))
    );
    assert!(!holds(&learner, "s-old").await);
}

#[tokio::test]
async fn everything_fetches_the_whole_sent_history() {
    let (learner, windows, _) = learner().await;
    review_range(&learner, LearnRange::Everything).await;

    let (state, older_not_fetched) = reviewed(&learner);
    assert!(matches!(state, Review::Ready(report) if report.found == 2));
    assert!(!older_not_fetched);
    assert!(
        windows
            .lock()
            .unwrap()
            .iter()
            .any(|asked| asked.window_for(&sent_mailbox()) == SyncWindow::full())
    );
    assert!(!holds(&learner, "s-old").await);
}

#[tokio::test]
async fn a_range_the_device_holds_fetches_nothing() {
    let (learner, windows, _) = learner().await;
    let passes = windows.lock().unwrap().len();
    let since = today() - Duration::days(10);
    review_range(
        &learner,
        LearnRange::Between {
            since,
            until: today(),
        },
    )
    .await;

    let (state, older_not_fetched) = reviewed(&learner);
    assert!(matches!(state, Review::Ready(report) if report.found == 1));
    assert!(!older_not_fetched);
    assert_eq!(windows.lock().unwrap().len(), passes, "no pass was run");
}

/// The server cannot be reached: the review reads what the device holds and says the older mail
/// could not be fetched.
#[tokio::test]
async fn a_fetch_that_fails_reads_what_the_device_holds_and_says_so() {
    let (learner, _, fail) = learner().await;
    // What the device holds is read from its cache, as it is offline.
    learner.app.prefetch_account_bodies(&account()).await;
    fail.store(true, Ordering::SeqCst);
    let since = today() - Duration::days(500);
    review_range(
        &learner,
        LearnRange::Between {
            since,
            until: today(),
        },
    )
    .await;

    let (state, older_not_fetched) = reviewed(&learner);
    assert!(matches!(state, Review::Ready(report) if report.found == 1));
    assert!(older_not_fetched);
}

/// While a review holds Sent further back, every pass of the account syncs under the deeper
/// windows, so a poll neither fights the fetch nor reconciles the older mail away; once it lets
/// go, passes are back on the account's own window.
#[tokio::test]
async fn a_pass_while_sent_is_held_further_back_syncs_under_the_deeper_windows() {
    let (learner, windows, _) = learner().await;
    let since = today() - Duration::days(500);
    learner.app.held_deeper().insert(
        account(),
        (vec![sent_mailbox()], SyncWindow::since(calendar_day(since))),
    );
    learner.app.refresh_account(&account()).await;
    let last = windows.lock().unwrap().last().cloned().unwrap();
    assert_eq!(
        last.window_for(&sent_mailbox()),
        SyncWindow::since(calendar_day(since))
    );
    assert!(holds(&learner, "s-old").await);

    learner.app.release_older_sent(&account()).await;
    assert!(!holds(&learner, "s-old").await);
    let last = windows.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last.deepened().count(), 0);
}
