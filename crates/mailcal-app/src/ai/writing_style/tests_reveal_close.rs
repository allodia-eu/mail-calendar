//! Closing a reveal (`docs/ai.md` rules 12 and 15): after learning, the flow's Close and the
//! reveal's are one control, drawn alike and leaving the reveal the same way whichever arrives.

use super::{
    LearningIntent, LearningStage, LearningStep, StyleControl, WritingStyleChange,
    tests_support::{Learner, learn, sent},
};
use crate::ai::WritingStyleId;

/// A learner whose flow stands on the reveal of the style it learned.
async fn learned() -> (Learner, WritingStyleId) {
    let learner = Learner::new(&["acct-1"], vec![sent("s1")]).await;
    learner.press(LearningIntent::Open).await;
    learner.press(LearningIntent::Next).await;
    learner.press(learn()).await;
    let LearningStage::Learned { style } = learner.view().stage else {
        panic!("not learned: {:?}", learner.view().stage);
    };
    (learner, style)
}

fn reveal_close(learner: &Learner, style: &WritingStyleId) -> StyleControl {
    learner.app.writing_style_reveal(style).unwrap().close
}

#[tokio::test]
async fn the_flows_close_is_the_reveals_close_after_learning() {
    let (learner, style) = learned().await;
    assert_eq!(learner.view().close, reveal_close(&learner, &style));

    let LearningStep::Change(change) = learner.intent(LearningIntent::Close).await else {
        panic!("the flow's Close leaves the reveal");
    };
    assert_eq!(
        change,
        WritingStyleChange::CloseReveal { id: style.clone() }
    );
    assert!(learner.app.queue_writing_style_change(change));
    let running = StyleControl {
        enabled: false,
        running: true,
    };
    assert_eq!(
        learner.view().close,
        running,
        "the host stays until the reveal is written as shown"
    );
    assert_eq!(reveal_close(&learner, &style), running);
    assert!(
        !learner
            .app
            .queue_writing_style_change(WritingStyleChange::CloseReveal { id: style.clone() }),
        "the reveal's Close pressed as well is the same change, made once"
    );

    learner.app.run_writing_style_changes();
    assert!(learner.app.writing_style_learning().is_none());
    assert!(learner.app.writing_style(&style).unwrap().revealed);
}

#[tokio::test]
async fn the_reveals_close_ends_the_flow_it_follows() {
    let (learner, style) = learned().await;
    learner
        .app
        .change_writing_style_now(WritingStyleChange::CloseReveal { id: style.clone() });
    assert!(learner.app.writing_style_learning().is_none());
    assert!(learner.app.writing_style(&style).unwrap().revealed);
    assert!(
        matches!(
            learner.intent(LearningIntent::Close).await,
            LearningStep::Nothing
        ),
        "a Close arriving after it finds nothing left to close"
    );
}
