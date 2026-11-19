//! The flow's stages, one cell of the contract's table at a time (`docs/ai.md` rule 12), and the
//! tickets that drop an answer for a review the person has left.

use engine_api::AccountId;
use mailcal_ai::corpus::{Corpus, CorpusReport};

use super::{Flows, LearningIntent, Review, Reviewed, Stage, Start};
use crate::ai::{AiFailure, writing_style::range::LearnRange};

fn accounts(ids: &[&str]) -> Vec<(AccountId, String)> {
    ids.iter()
        .map(|id| (AccountId::try_from(*id).unwrap(), format!("me@{id}.eu")))
        .collect()
}

fn apply(flows: &mut Flows, intent: LearningIntent, ids: &[&str]) -> Start {
    flows.apply(intent, &accounts(ids), |_, _| true, Ok(()))
}

fn stage(flows: &Flows) -> Option<Stage> {
    flows.flow.as_ref().map(|flow| flow.stage.clone())
}

fn learn() -> LearningIntent {
    LearningIntent::Learn {
        name: "My style".to_owned(),
        language: "en".to_owned(),
    }
}

/// A flow at `Review` with something read, over two accounts.
fn reviewed() -> Flows {
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a", "b"]);
    apply(
        &mut flows,
        LearningIntent::SelectAccount {
            account: "a".to_owned(),
        },
        &["a", "b"],
    );
    apply(&mut flows, LearningIntent::Next, &["a", "b"]);
    let Start::Read(ticket) = apply(&mut flows, LearningIntent::Next, &["a", "b"]) else {
        panic!("Next reads");
    };
    let flow = flows.current(ticket).unwrap();
    let report = CorpusReport {
        usable: 3,
        ..CorpusReport::default()
    };
    flow.stage = Stage::Review(Review::Ready(report.clone()));
    flow.reviewed = Some(Reviewed {
        corpus: Corpus {
            languages: std::collections::BTreeMap::new(),
            report,
        },
        keyword_kept: true,
    });
    flows
}

#[test]
fn one_account_opens_on_the_range_and_several_on_the_account() {
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    assert_eq!(stage(&flows), Some(Stage::ChooseRange));
    assert!(!flows.flow.as_ref().unwrap().asked_account);

    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a", "b"]);
    assert_eq!(stage(&flows), Some(Stage::ChooseAccount));
    apply(&mut flows, LearningIntent::Next, &["a", "b"]);
    assert_eq!(
        stage(&flows),
        Some(Stage::ChooseAccount),
        "Next waits for a choice"
    );

    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &[]);
    assert!(flows.flow.is_none(), "no account used for mail, no flow");
}

#[test]
fn back_from_each_stage_goes_where_the_table_says() {
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a", "b"]);
    apply(&mut flows, LearningIntent::Back, &["a", "b"]);
    assert_eq!(
        stage(&flows),
        Some(Stage::ChooseAccount),
        "ChooseAccount has none"
    );

    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    apply(&mut flows, LearningIntent::Back, &["a"]);
    assert_eq!(
        stage(&flows),
        Some(Stage::ChooseRange),
        "nowhere back to when the account was not asked"
    );

    let mut flows = reviewed();
    apply(&mut flows, LearningIntent::Back, &["a", "b"]);
    assert_eq!(stage(&flows), Some(Stage::ChooseRange));
    apply(&mut flows, LearningIntent::Back, &["a", "b"]);
    assert_eq!(
        stage(&flows),
        Some(Stage::ChooseAccount),
        "where it was shown"
    );

    let mut flows = reviewed();
    apply(&mut flows, learn(), &["a", "b"]);
    apply(&mut flows, LearningIntent::Back, &["a", "b"]);
    assert!(
        matches!(stage(&flows), Some(Stage::Learning { .. })),
        "Learning has none"
    );
}

#[test]
fn back_from_a_failure_goes_to_the_stage_its_remedy_names_otherwise_review() {
    let cases = [
        (
            AiFailure::NoSentFolder {
                another_account: true,
            },
            Stage::ChooseAccount,
        ),
        (AiFailure::NothingToLearn, Stage::ChooseRange),
        (
            AiFailure::Unreachable,
            Stage::Review(Review::Ready(CorpusReport {
                usable: 3,
                ..CorpusReport::default()
            })),
        ),
    ];
    for (failure, back_to) in cases {
        let mut flows = reviewed();
        flows.flow.as_mut().unwrap().stage = Stage::Failed(failure);
        apply(&mut flows, LearningIntent::Back, &["a", "b"]);
        assert_eq!(stage(&flows), Some(back_to), "{failure:?}");
    }
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    flows.flow.as_mut().unwrap().stage = Stage::Failed(AiFailure::NowhereToSend);
    apply(&mut flows, LearningIntent::Back, &["a"]);
    assert_eq!(
        stage(&flows),
        Some(Stage::ChooseRange),
        "nothing was read to return to"
    );
}

#[test]
fn closing_ends_the_flow_except_while_learning_and_marks_a_reveal_shown() {
    for setup in [
        |flows: &mut Flows| apply(flows, LearningIntent::Open, &["a", "b"]),
        |flows: &mut Flows| apply(flows, LearningIntent::Open, &["a"]),
    ] {
        let mut flows = Flows::default();
        setup(&mut flows);
        apply(&mut flows, LearningIntent::Close, &["a", "b"]);
        assert!(flows.flow.is_none());
    }
    let mut flows = reviewed();
    apply(&mut flows, LearningIntent::Close, &["a", "b"]);
    assert!(flows.flow.is_none(), "Review ends");

    let mut flows = reviewed();
    flows.flow.as_mut().unwrap().stage = Stage::Failed(AiFailure::Unreachable);
    apply(&mut flows, LearningIntent::Close, &["a", "b"]);
    assert!(flows.flow.is_none(), "Failed ends");

    let mut flows = reviewed();
    let Start::Learn(ticket) = apply(&mut flows, learn(), &["a", "b"]) else {
        panic!("Learn runs");
    };
    apply(&mut flows, LearningIntent::Close, &["a", "b"]);
    let flow = flows
        .current(ticket)
        .expect("the run goes on under its ticket");
    assert!(!flow.shown);
    apply(&mut flows, LearningIntent::Open, &["a", "b"]);
    assert!(
        flows.current(ticket).unwrap().shown,
        "reopening joins the run"
    );

    let mut flows = reviewed();
    let id = mailcal_account::WritingStyleId::new("s1").unwrap();
    flows.flow.as_mut().unwrap().stage = Stage::Learned(id.clone());
    assert!(matches!(
        apply(&mut flows, LearningIntent::Close, &["a", "b"]),
        Start::RevealClosed(closed) if closed == id
    ));
    assert!(flows.flow.is_none());
}

#[test]
fn stop_returns_to_the_review_and_drops_the_runs_answer() {
    let mut flows = reviewed();
    let Start::Learn(ticket) = apply(&mut flows, learn(), &["a", "b"]) else {
        panic!("Learn runs");
    };
    apply(&mut flows, LearningIntent::Stop, &["a", "b"]);
    assert!(matches!(
        stage(&flows),
        Some(Stage::Review(Review::Ready(_)))
    ));
    assert!(
        flows.current(ticket).is_none(),
        "the run's answer lands nowhere"
    );

    let mut flows = reviewed();
    apply(&mut flows, learn(), &["a", "b"]);
    apply(&mut flows, LearningIntent::Close, &["a", "b"]);
    apply(&mut flows, LearningIntent::Stop, &["a", "b"]);
    assert!(
        flows.flow.is_none(),
        "stopped from the page, nothing to return to"
    );
}

#[test]
fn a_read_left_by_going_back_or_choosing_again_lands_nowhere() {
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    let Start::Read(first) = apply(&mut flows, LearningIntent::Next, &["a"]) else {
        panic!("Next reads");
    };
    apply(&mut flows, LearningIntent::Back, &["a"]);
    assert!(flows.current(first).is_none());
    let until = LearnRange::UntilEndOfLastYear;
    apply(
        &mut flows,
        LearningIntent::SelectRange { range: until },
        &["a"],
    );
    let Start::Read(second) = apply(&mut flows, LearningIntent::Next, &["a"]) else {
        panic!("Next reads again");
    };
    assert!(second > first);
    assert_eq!(flows.current(second).unwrap().range, Some(until));
}

#[test]
fn nothing_is_read_while_there_is_nowhere_to_send() {
    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    let start = flows.apply(
        LearningIntent::Next,
        &accounts(&["a"]),
        |_, _| true,
        Err(AiFailure::NowhereToSend),
    );
    assert!(matches!(start, Start::Nothing));
    assert_eq!(stage(&flows), Some(Stage::Failed(AiFailure::NowhereToSend)));
}

#[test]
fn learn_needs_something_to_learn_from_and_a_range_the_account_can_honour() {
    let mut flows = reviewed();
    flows.flow.as_mut().unwrap().stage =
        Stage::Review(Review::NothingUsable(CorpusReport::default()));
    assert!(matches!(
        apply(&mut flows, learn(), &["a", "b"]),
        Start::Nothing
    ));

    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    flows.apply(
        LearningIntent::SelectRange {
            range: LearnRange::UntilEndOfLastYear,
        },
        &accounts(&["a"]),
        |_, range| range == LearnRange::Everything,
        Ok(()),
    );
    assert_eq!(
        flows.flow.as_ref().unwrap().range,
        Some(LearnRange::Everything),
        "a range the account cannot honour is not taken"
    );
}

#[test]
fn try_again_reruns_what_failed() {
    let mut flows = reviewed();
    apply(&mut flows, learn(), &["a", "b"]);
    let flow = flows.flow.as_mut().unwrap();
    flow.stage = Stage::Failed(AiFailure::Unreachable);
    flow.run_failed = true;
    assert!(matches!(
        apply(&mut flows, LearningIntent::Remedy, &["a", "b"]),
        Start::Learn(_)
    ));

    let mut flows = Flows::default();
    apply(&mut flows, LearningIntent::Open, &["a"]);
    flows.flow.as_mut().unwrap().stage = Stage::Failed(AiFailure::Unreachable);
    assert!(matches!(
        apply(&mut flows, LearningIntent::Remedy, &["a"]),
        Start::Read(_)
    ));
}
