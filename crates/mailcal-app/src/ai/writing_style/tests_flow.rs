//! The learning flow through the app: reading the Sent folder, the consent, the run, Stop, Close
//! during a run, and the failures each stage can come to.

use engine_api::MessageIdHeader;
use mailcal_account::drafted_reply_keyword;

use super::{
    LearningIntent, LearningStage, LearningStep, PageRun, Review, StyleOpens,
    tests_support::{Answer, Learner, learn, sent},
};
use crate::{
    Surface,
    ai::{AiFailure, AiRemedy, AiRoute, RemedyControl},
    tests_fakes::FakeProvider,
};

fn review(learner: &Learner) -> Review {
    match learner.view().stage {
        LearningStage::Review { state, .. } => state,
        other => panic!("not on Review: {other:?}"),
    }
}

#[tokio::test]
async fn one_account_learns_from_its_sent_mail_after_the_consent() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    let view = learner.view();
    assert!(matches!(view.stage, LearningStage::ChooseRange { .. }));
    assert!(view.back.is_none(), "the account was not asked");
    assert!(view.next.is_some_and(|next| next.enabled));

    learner.press(LearningIntent::Next).await;
    let view = learner.view();
    let LearningStage::Review {
        address,
        state: Review::Ready(report),
        sends_to,
        keyword_kept,
        ..
    } = view.stage
    else {
        panic!("not ready: {:?}", view.stage);
    };
    assert_eq!(address, "me@acct-1.local");
    assert_eq!((report.found, report.usable), (1, 1));
    assert_eq!(report.languages[0].language.as_str(), "en");
    assert_eq!(
        sends_to,
        Some(Ok(AiRoute::OwnEndpoint {
            host: "models.example.eu".to_owned(),
            declared: Some(mailcal_ai::Class::EuNative),
        }))
    );
    assert!(!keyword_kept, "the fake keeps no keyword on a Sent copy");
    assert_eq!(
        *learner.host.asked.lock().unwrap(),
        0,
        "nothing left before Learn"
    );
    assert!(view.learn.is_some_and(|learn| learn.enabled));

    learner.press(learn()).await;
    let LearningStage::Learned { style } = learner.view().stage else {
        panic!("not learned: {:?}", learner.view().stage);
    };
    let stored = learner.app.writing_style(&style).unwrap();
    assert_eq!(stored.name, "My style");
    assert_eq!(stored.learned_from, "me@acct-1.local");
    assert!(!stored.revealed);
    assert_eq!(
        learner.app.writing_style_slot("acct-1"),
        Some(style.clone())
    );
    assert!(
        learner
            .surfaces
            .lock()
            .unwrap()
            .contains(&Surface::WritingStyle)
    );

    learner.press(LearningIntent::Close).await;
    assert!(learner.app.writing_style_learning().is_none());
    assert!(
        learner.app.writing_style(&style).unwrap().revealed,
        "closing the reveal counts it as shown"
    );
}

/// `docs/ai.md` rules 14 and 25: a reply drafted for the person is never learned from, whether
/// it carries the keyword or only this device logged it.
#[tokio::test]
async fn drafted_replies_are_left_out_of_what_is_read() {
    let mut keyword = sent("s2");
    keyword.keywords.insert(drafted_reply_keyword());
    let mut logged = sent("s3");
    logged.envelope.message_id = vec![MessageIdHeader::new("<logged@acct-1.local>").unwrap()];
    let learner = Learner::new(&["acct-1"], vec![sent("s1"), keyword, logged]).await;
    learner.app.note_drafted_reply(
        "acct-1",
        MessageIdHeader::new("<logged@acct-1.local>").unwrap(),
    );
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    let Review::Ready(report) = review(&learner) else {
        panic!("nothing ready");
    };
    assert_eq!(report.found, 1, "only the person's own reply is read");
}

#[tokio::test]
async fn several_accounts_ask_which_and_go_back_to_the_question() {
    let learner = Learner::new(&["acct-1", "acct-2"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    let LearningStage::ChooseAccount { accounts, selected } = learner.view().stage else {
        panic!("not asked");
    };
    assert_eq!(accounts.len(), 2);
    assert_eq!(selected, None);
    learner
        .press(LearningIntent::SelectAccount {
            account: "acct-1".to_owned(),
        })
        .await;
    learner.press(LearningIntent::Next).await;
    assert!(learner.view().back.is_some_and(|back| back.enabled));
    learner.press(LearningIntent::Back).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::ChooseAccount {
            selected: Some(_),
            ..
        }
    ));
}

#[tokio::test]
async fn an_answer_for_a_review_the_person_left_is_dropped() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    let LearningStep::Read(ticket) = learner
        .app
        .writing_style_learning_intent(LearningIntent::Next)
    else {
        panic!("Next reads");
    };
    assert_eq!(review(&learner), Review::Reading);
    learner.press(LearningIntent::Back).await;
    learner.app.read_learning_review(ticket).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::ChooseRange { .. }
    ));
}

#[tokio::test]
async fn a_failed_run_offers_its_remedy_and_try_again_learns() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    *learner.host.answer.lock().unwrap() = Answer::Unavailable;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    learner.press(learn()).await;
    let view = learner.view();
    assert_eq!(
        view.stage,
        LearningStage::Failed {
            failure: AiFailure::ServerError,
            remedy: Some(RemedyControl {
                remedy: AiRemedy::TryAgain,
                enabled: true,
            }),
        }
    );
    assert!(view.back.is_some());

    *learner.host.answer.lock().unwrap() = Answer::Describe;
    learner.press(LearningIntent::Remedy).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::Learned { .. }
    ));
}

#[tokio::test]
async fn an_empty_sent_folder_has_nothing_usable_and_learn_is_held() {
    let learner = Learner::new(&["acct-1"], Vec::new()).await;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    let Review::NothingUsable(report) = review(&learner) else {
        panic!("an empty Sent folder has nothing usable");
    };
    assert_eq!(report.found, 0);
    let view = learner.view();
    assert!(view.learn.is_some_and(|learn| !learn.enabled));
    let LearningStage::Review { sends_to, .. } = view.stage else {
        panic!("not on Review");
    };
    assert_eq!(sends_to, None, "nothing to send names no destination");
}

#[tokio::test]
async fn an_account_without_a_sent_folder_fails_and_offers_another() {
    let learner = Learner::over(vec![
        ("acct-1", FakeProvider::new()),
        ("acct-2", FakeProvider::with_sent_and_archive(Vec::new())),
    ])
    .await;
    learner.press(LearningIntent::Open).await;
    learner
        .press(LearningIntent::SelectAccount {
            account: "acct-1".to_owned(),
        })
        .await;
    learner.press(LearningIntent::Next).await;
    learner.press(LearningIntent::Next).await;
    let failure = AiFailure::NoSentFolder {
        another_account: true,
    };
    assert_eq!(
        learner.view().stage,
        LearningStage::Failed {
            failure,
            remedy: failure.remedy_control(true),
        }
    );
    learner.press(LearningIntent::Remedy).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::ChooseAccount { .. }
    ));
}

#[tokio::test]
async fn nowhere_to_send_reads_nothing_and_offers_the_endpoint() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner
        .app
        .change_ai_endpoint_now(crate::ai::AiEndpointChange::Remove);
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    assert_eq!(
        learner.view().stage,
        LearningStage::Failed {
            failure: AiFailure::NowhereToSend,
            remedy: AiFailure::NowhereToSend.remedy_control(true),
        }
    );
    assert_eq!(
        AiFailure::NowhereToSend.remedy(),
        Some(AiRemedy::OpenAiEndpoint)
    );
}

/// Stop ends the run before its next request and keeps nothing; closing during a run leaves it
/// going, drawn on the page, and a style it learns then opens the reveal from its row.
#[tokio::test]
async fn stop_keeps_nothing_and_closing_leaves_the_run_on_the_page() {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    let (entered, release) = learner.host.hold();
    let LearningStep::Learn(work) = learner.intent(learn()).await else {
        panic!("Learn queues the run");
    };
    let app = std::sync::Arc::clone(&learner.app);
    let runner = std::thread::spawn(move || app.run_ai_work_now(work));
    entered.recv().unwrap();
    learner.press(LearningIntent::Stop).await;
    assert!(matches!(
        learner.view().stage,
        LearningStage::Review {
            state: Review::Ready(_),
            ..
        }
    ));
    release.send(()).unwrap();
    runner.join().unwrap();
    assert!(
        learner.app.writing_styles().is_empty(),
        "a stopped run keeps nothing"
    );

    let LearningStep::Learn(work) = learner.intent(learn()).await else {
        panic!("Learn queues the run");
    };
    let app = std::sync::Arc::clone(&learner.app);
    let runner = std::thread::spawn(move || app.run_ai_work_now(work));
    entered.recv().unwrap();
    learner.press(LearningIntent::Close).await;
    assert!(
        learner.app.writing_style_learning().is_none(),
        "the host closed"
    );
    let page = learner.app.writing_style_page().unwrap();
    assert!(matches!(
        page.run,
        Some(PageRun::Learning { stop, .. }) if stop.enabled
    ));
    assert!(!page.learn.enabled);
    release.send(()).unwrap();
    runner.join().unwrap();
    let page = learner.app.writing_style_page().unwrap();
    assert_eq!(page.run, None);
    assert_eq!(page.styles.len(), 1);
    assert_eq!(page.styles[0].opens, StyleOpens::Reveal);
}
