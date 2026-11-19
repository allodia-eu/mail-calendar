//! The learning flow's trail (`docs/ai.md` rule 12): the stages before the current one, each where
//! Back from the one after it goes, so a host that draws the flow as a navigation stack keys its
//! path on the record and never rebuilds it.

use LearningTrailStage::{ChooseAccount, ChooseRange, Review};

use super::{
    LearningIntent, LearningStage, LearningTrailStage,
    tests_support::{Answer, Learner, learn, sent},
};
use crate::{ai::AiFailure, tests_fakes::FakeProvider};

fn trail(learner: &Learner) -> Vec<LearningTrailStage> {
    learner.view().trail
}

/// Back goes to the last stage of the trail, and leaves the trail without it.
async fn back_pops(learner: &Learner) {
    let mut before = trail(learner);
    let to = before.pop().expect("a stage with a Back has a trail");
    learner.press(LearningIntent::Back).await;
    let stage = learner.view().stage;
    let landed = match stage {
        LearningStage::ChooseAccount { .. } => ChooseAccount,
        LearningStage::ChooseRange { .. } => ChooseRange,
        LearningStage::Review { .. } => Review,
        other => panic!("Back landed on {other:?}"),
    };
    assert_eq!(landed, to, "Back goes where the trail says");
    assert_eq!(trail(learner), before);
}

#[tokio::test]
async fn one_account_starts_on_the_range_with_nothing_behind_it() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    assert_eq!(trail(&learner), []);
    assert!(learner.view().back.is_none());

    learner.press(LearningIntent::Next).await;
    assert!(matches!(learner.view().stage, LearningStage::Review { .. }));
    assert_eq!(trail(&learner), [ChooseRange]);
    back_pops(&learner).await;

    learner.press(LearningIntent::Next).await;
    learner.press(learn()).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::Learned { .. }
    ));
    assert_eq!(trail(&learner), [ChooseRange, Review]);
}

#[tokio::test]
async fn several_accounts_start_on_the_account_and_keep_it_in_the_trail() {
    let learner = Learner::new(&["acct-1", "acct-2"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    assert_eq!(trail(&learner), []);
    learner
        .press(LearningIntent::SelectAccount {
            account: "acct-1".to_owned(),
        })
        .await;
    assert_eq!(trail(&learner), [], "choosing moves nothing");

    learner.press(LearningIntent::Next).await;
    assert_eq!(trail(&learner), [ChooseAccount]);
    learner.press(LearningIntent::Next).await;
    assert_eq!(trail(&learner), [ChooseAccount, ChooseRange]);
    back_pops(&learner).await;
    back_pops(&learner).await;
    assert_eq!(trail(&learner), []);

    learner.press(LearningIntent::Next).await;
    learner.press(LearningIntent::Next).await;
    learner.press(learn()).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::Learned { .. }
    ));
    assert_eq!(trail(&learner), [ChooseAccount, ChooseRange, Review]);
}

/// A run stands on the review it was started from, and Stop returns there.
#[tokio::test]
async fn a_run_stands_on_its_review() {
    let learner = Learner::new(&["acct-1", "acct-2"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    learner
        .press(LearningIntent::SelectAccount {
            account: "acct-1".to_owned(),
        })
        .await;
    learner.press(LearningIntent::Next).await;
    learner.press(LearningIntent::Next).await;
    let (entered, release) = learner.host.hold();
    let super::LearningStep::Learn(work) = learner.intent(learn()).await else {
        panic!("Learn queues the run");
    };
    let app = std::sync::Arc::clone(&learner.app);
    let runner = std::thread::spawn(move || app.run_ai_work_now(work));
    entered.recv().unwrap();
    assert!(matches!(
        learner.view().stage,
        LearningStage::Learning { .. }
    ));
    assert_eq!(trail(&learner), [ChooseAccount, ChooseRange, Review]);
    learner.press(LearningIntent::Stop).await;
    assert!(matches!(learner.view().stage, LearningStage::Review { .. }));
    assert_eq!(trail(&learner), [ChooseAccount, ChooseRange]);
    release.send(()).unwrap();
    runner.join().unwrap();
}

/// A failure stands on the stage its Back goes to: the review for a run that failed, the range
/// when nothing was read, the account for one with no Sent folder.
#[tokio::test]
async fn a_failure_stands_where_its_back_goes() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    *learner.host.answer.lock().unwrap() = Answer::Unavailable;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    learner.press(learn()).await;
    assert!(matches!(learner.view().stage, LearningStage::Failed { .. }));
    assert_eq!(trail(&learner), [ChooseRange, Review]);
    back_pops(&learner).await;

    let nowhere = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    nowhere
        .app
        .change_ai_endpoint_now(crate::ai::AiEndpointChange::Remove);
    nowhere.press(LearningIntent::Open).await;
    nowhere.press(LearningIntent::Next).await;
    assert!(matches!(
        nowhere.view().stage,
        LearningStage::Failed {
            failure: AiFailure::NowhereToSend,
            ..
        }
    ));
    assert_eq!(trail(&nowhere), [ChooseRange]);
    back_pops(&nowhere).await;

    let no_sent = Learner::over(vec![
        ("acct-1", FakeProvider::new()),
        ("acct-2", FakeProvider::with_sent_and_archive(Vec::new())),
    ])
    .await;
    no_sent.press(LearningIntent::Open).await;
    no_sent
        .press(LearningIntent::SelectAccount {
            account: "acct-1".to_owned(),
        })
        .await;
    no_sent.press(LearningIntent::Next).await;
    no_sent.press(LearningIntent::Next).await;
    assert!(matches!(
        no_sent.view().stage,
        LearningStage::Failed {
            failure: AiFailure::NoSentFolder {
                another_account: true
            },
            ..
        }
    ));
    assert_eq!(trail(&no_sent), [ChooseAccount]);
    back_pops(&no_sent).await;
}
