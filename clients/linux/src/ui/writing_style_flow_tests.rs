//! Where the learning sheet stands, pinned without a window.

use mailcal_bindings::{
    AiRoute, CorpusLanguage, CorpusReport, LearningProgress, LearningStage, WritingStyleFailure,
};

use super::{
    LearnAction, LearnFlow, LearnPage, LearnRequest, LearnStage, Range, consent_text,
    progress_text, report_lines,
};
use crate::{l10n, ui::writing_style::languages_text};

fn report(usable: u32) -> CorpusReport {
    CorpusReport {
        found: 12,
        usable,
        undetected: 0,
        languages: vec![CorpusLanguage {
            language: "en".to_owned(),
            usable,
            sampled: usable,
            tokens: 900,
        }],
        oldest: None,
        newest: None,
        horizon: None,
    }
}

fn progress(account: &str, stage: LearningStage, done: u32, total: u32) -> LearningProgress {
    LearningProgress {
        account_id: account.to_owned(),
        stage,
        done,
        total,
    }
}

/// One account is not a question worth asking: the sheet opens on its report.
#[test]
fn a_single_account_skips_the_account_question() {
    let (flow, request) = LearnFlow::new(&["work".to_owned()]);
    assert_eq!(flow.stage(), &LearnStage::Reading);
    let request = request.expect("the report is read at once");
    assert_eq!(request.account, "work");
    assert_eq!(request.range, Range::default(), "everything on the device");

    let (flow, request) = LearnFlow::new(&["work".to_owned(), "home".to_owned()]);
    assert_eq!(flow.stage(), &LearnStage::Account);
    assert!(
        request.is_none(),
        "nothing is read before an account is chosen"
    );
}

/// The sheet's pages follow the same order, and the run comes last.
#[test]
fn the_account_page_is_there_only_when_there_is_a_choice() {
    assert_eq!(
        LearnPage::all(1),
        [LearnPage::Range, LearnPage::Consent, LearnPage::Progress]
    );
    assert_eq!(
        LearnPage::all(2),
        [
            LearnPage::Account,
            LearnPage::Range,
            LearnPage::Consent,
            LearnPage::Progress
        ]
    );
    assert!(LearnPage::Consent.can_go_back());
    assert!(!LearnPage::Progress.can_go_back(), "Stop is the way out");
}

/// Going back to the account page and choosing another reads the device for that one, until the
/// sample has gone.
#[test]
fn the_account_can_change_until_the_run_starts() {
    let (mut flow, _) = LearnFlow::new(&["work".to_owned(), "home".to_owned()]);
    assert_eq!(
        flow.action(LearnPage::Account),
        LearnAction::Next { enabled: false }
    );
    let work = flow.choose_account("work".to_owned()).expect("a read");
    assert_eq!(
        flow.action(LearnPage::Account),
        LearnAction::Next { enabled: true }
    );
    let home = flow
        .choose_account("home".to_owned())
        .expect("a second read");
    assert_eq!(home.account, "home");
    flow.report_arrived(work.ticket, Ok(report(4)));
    assert_eq!(flow.stage(), &LearnStage::Reading, "work's report is stale");
    flow.report_arrived(home.ticket, Ok(report(4)));
    flow.consent().expect("consented");
    assert!(flow.choose_account("work".to_owned()).is_none());
}

/// Learn stands under a report with something in it, Stop under a run, Close under one that
/// ended without a style.
#[test]
fn each_page_offers_what_its_stage_allows() {
    let (mut flow, request) = LearnFlow::new(&["work".to_owned()]);
    assert_eq!(flow.action(LearnPage::Consent), LearnAction::Nothing);
    let request = request.expect("a read");
    flow.report_arrived(request.ticket, Ok(report(0)));
    assert_eq!(
        flow.action(LearnPage::Consent),
        LearnAction::Nothing,
        "nothing to learn from"
    );
    let again = flow.choose_range(Range::default()).expect("a read");
    flow.report_arrived(again.ticket, Ok(report(4)));
    assert_eq!(flow.action(LearnPage::Consent), LearnAction::Learn);
    assert_eq!(flow.action(LearnPage::Progress), LearnAction::Nothing);
    flow.consent().expect("consented");
    assert_eq!(flow.action(LearnPage::Progress), LearnAction::Stop);
    flow.finished(Err(WritingStyleFailure::Cancelled));
    assert_eq!(flow.action(LearnPage::Progress), LearnAction::Close);
}

/// Changing the range reads the device again, and a report for the range left behind lands
/// nowhere: it would describe mail the person has already decided not to send.
#[test]
fn only_the_latest_reads_answer_lands() {
    let (mut flow, first) = LearnFlow::new(&["work".to_owned()]);
    let first = first.expect("a first read");
    let until = Range {
        since: None,
        until: Some(1_735_689_599),
    };
    let second = flow.choose_range(until).expect("a second read");
    assert_eq!(second.range, until);

    flow.report_arrived(first.ticket, Ok(report(3)));
    assert_eq!(
        flow.stage(),
        &LearnStage::Reading,
        "the stale answer is dropped"
    );
    flow.report_arrived(second.ticket, Ok(report(5)));
    assert_eq!(flow.stage(), &LearnStage::Report(report(5)));
}

/// Pressing Learn is the consent, and it consents to exactly the account and range the report
/// described. With nothing to learn from there is nothing to consent to.
#[test]
fn consent_sends_what_the_report_described_and_nothing_else() {
    let (mut flow, request) = LearnFlow::new(&["work".to_owned()]);
    let request = request.expect("a read");
    flow.report_arrived(request.ticket, Ok(report(0)));
    assert_eq!(flow.consent(), None);

    let until = Range::up_to(2024, 12, 31, "UTC").expect("a real day");
    let reread = flow
        .choose_range(until)
        .expect("the range can still change");
    flow.report_arrived(reread.ticket, Ok(report(4)));
    assert_eq!(
        flow.consent(),
        Some(LearnRequest {
            account: "work".to_owned(),
            range: until,
        })
    );
    assert!(flow.is_learning());
    assert!(
        flow.choose_range(Range::default()).is_none(),
        "the sample has gone; its range cannot change under it"
    );
    assert_eq!(flow.consent(), None, "one consent, one run");
}

/// The range ends at the last second of the chosen day where the reader is, so "until the end of
/// 2024" keeps the evening of the 31st.
#[test]
fn a_range_ends_at_the_end_of_its_day_in_the_display_zone() {
    let utc = Range::up_to(2024, 12, 31, "UTC").expect("a real day");
    assert_eq!(utc.until, Some(1_735_689_599));
    assert_eq!(utc.since, None);
    let amsterdam = Range::up_to(2024, 12, 31, "Europe/Amsterdam").expect("a real day");
    assert_eq!(amsterdam.until, Some(1_735_689_599 - 3600));
    assert_eq!(Range::up_to(2024, 2, 30, "UTC"), None);
}

/// Progress is this run's only, and the run's answer ends it either way.
#[test]
fn a_run_takes_its_own_progress_and_then_its_answer() {
    let (mut flow, request) = LearnFlow::new(&["work".to_owned()]);
    flow.report_arrived(request.expect("a read").ticket, Ok(report(4)));
    flow.consent().expect("consented");

    flow.progress(Some(&progress("home", LearningStage::Learning, 1, 2)));
    assert_eq!(
        flow.stage(),
        &LearnStage::Learning(None),
        "another account's run"
    );
    let own = progress("work", LearningStage::Learning, 1, 2);
    flow.progress(Some(&own));
    assert_eq!(flow.stage(), &LearnStage::Learning(Some(own)));
    flow.progress(None);
    assert!(
        flow.is_learning(),
        "a run's end is its answer, not a missing progress"
    );

    flow.finished(Ok("style-1".to_owned()));
    assert_eq!(flow.learned(), Some("style-1"));
}

#[test]
fn a_stopped_run_is_a_failure_the_sheet_words() {
    let (mut flow, request) = LearnFlow::new(&["work".to_owned()]);
    flow.report_arrived(request.expect("a read").ticket, Ok(report(4)));
    flow.consent().expect("consented");
    flow.finished(Err(WritingStyleFailure::Cancelled));
    assert_eq!(
        flow.stage(),
        &LearnStage::Failed(WritingStyleFailure::Cancelled)
    );
    assert_eq!(flow.learned(), None);
    // An answer with no run behind it changes nothing.
    flow.finished(Ok("late".to_owned()));
    assert_eq!(flow.learned(), None);
}

#[test]
fn the_report_states_only_what_there_is_to_say() {
    let plain = report_lines(&report(4), "UTC", "en");
    assert_eq!(
        plain,
        [
            l10n::learn_report_found(12),
            l10n::learn_report_usable(4),
            l10n::learn_report_languages(&languages_text(&["en".to_owned()])),
        ]
    );

    let mut fuller = report(4);
    fuller.undetected = 2;
    fuller.horizon = Some(1_735_689_599);
    let lines = report_lines(&fuller, "UTC", "en");
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[3], l10n::learn_report_undetected(2));
    assert_eq!(lines[4], l10n::learn_report_horizon("31 Dec 2024"));
}

/// The consent names where the words go: Allodia's relay, or the host the person set up.
#[test]
fn the_consent_names_the_destination() {
    assert_eq!(
        consent_text(Some(AiRoute::Relay), None).as_deref(),
        Some(l10n::learn_consent_relay())
    );
    assert_eq!(
        consent_text(
            Some(AiRoute::OwnEndpoint),
            Some("http://127.0.0.1:28434/v1")
        ),
        Some(l10n::learn_consent_own("127.0.0.1"))
    );
    assert_eq!(
        consent_text(Some(AiRoute::OwnEndpoint), Some("https://ai.example.eu/v1")),
        Some(l10n::learn_consent_own("ai.example.eu"))
    );
    assert_eq!(
        consent_text(None, None),
        None,
        "nowhere to send, nothing to consent to"
    );
}

#[test]
fn reading_has_no_bar_and_learning_fills_one() {
    assert_eq!(
        progress_text(None),
        (l10n::learn_reading().to_owned(), None)
    );
    assert_eq!(
        progress_text(Some(&progress("work", LearningStage::Reading, 0, 0))),
        (l10n::learn_reading().to_owned(), None)
    );
    let (text, fraction) = progress_text(Some(&progress("work", LearningStage::Learning, 1, 4)));
    assert_eq!(text, l10n::learn_progress("1", "4"));
    assert!(fraction.is_some_and(|fraction| (fraction - 0.25).abs() < f64::EPSILON));
}
