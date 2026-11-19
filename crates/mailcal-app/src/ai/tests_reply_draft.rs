//! Draft a reply in the core (`docs/ai.md` rules 17 to 23): when it is offered and why it cannot
//! draft, the states a draft moves through, Stop, the replace question, the card following the
//! editor's reports and the person's ticks. Over the reply fixture's provider, with a relay that
//! answers from a script; nothing reaches the network.
//!
//! A child of the rich-submit tests, for [`super::mail_fake`].

use std::sync::Arc;

use mailcal_ai::{AiError, TaskKind};
use mailcal_composer::LeadText;

use super::{
    mail_fake::{ThreadProvider, app_and_logs, original_message, staging_dir},
    reply_draft_support::{
        RuntimeHost, ScriptedRelay, assistant, drafted, drafted_into, learn_a_style, open_reply,
        report, run, start, view,
    },
};
use crate::{
    App, ComposeOrigin, CompositionId, Intent,
    ai::{AiFailure, AiQueued, AiRemedy, BuildKind, ReplyDraftState},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_draft_moves_from_ready_through_drafting_to_the_card() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    app.take_reply_draft_report(&reply, report(0, LeadText::Empty, false, vec![], 0), 0);

    let ready = view(&app, &reply);
    assert_eq!(ready.state, ReplyDraftState::Ready);
    assert!(ready.control.enabled && ready.draft.enabled && !ready.stop.enabled);
    assert!(!ready.ask_before_replacing);

    assert_eq!(app.draft_reply(&reply, start(false)), Some(AiQueued::Run));
    let drafting = view(&app, &reply);
    assert_eq!(drafting.state, ReplyDraftState::Drafting);
    assert!(drafting.control.running && !drafting.control.enabled);
    assert!(!drafting.draft.enabled && drafting.stop.enabled);
    assert_eq!(drafting.instruction, "yes, Friday", "trimmed by the core");
    assert_eq!(
        app.draft_reply(&reply, start(false)),
        None,
        "one draft at a time"
    );

    run(&app).await;
    // Handed to the editor, not in yet: still drafting.
    let handed = view(&app, &reply);
    assert_eq!(handed.state, ReplyDraftState::Drafting);
    let script = handed.editor_script.expect("a draft for the editor");
    assert!(script.script.starts_with("window.setComposerDraftText("));
    assert!(script.script.contains(r#""gaps":["[time]","[amount]"]"#));
    assert!(script.script.contains(r#""insert":true"#));

    app.take_reply_draft_report(
        &reply,
        report(script.serial, LeadText::Draft, true, vec![0, 1], 1),
        0,
    );
    let card = view(&app, &reply);
    let ReplyDraftState::Drafted { summary, checklist } = card.state else {
        panic!("the card, not {:?}", card.state);
    };
    assert_eq!(summary, "Anna asks whether Friday still works.");
    let kinds: Vec<_> = checklist
        .iter()
        .map(|item| (item.kind, item.done, item.tick.enabled))
        .collect();
    assert_eq!(
        kinds,
        [
            (TaskKind::FillIn, false, false),
            (TaskKind::FillIn, false, false),
            (TaskKind::Attach, false, true),
            (TaskKind::Do, false, true),
        ]
    );
    // From now on the script only follows, for an editor opened later on the composition.
    assert!(
        card.editor_script
            .unwrap()
            .script
            .contains(r#""insert":false"#)
    );
    // The request went out with the instruction, and never with the app's own words.
    let seen = relay.seen.lock().unwrap();
    assert!(seen[0].messages[1].content.contains("yes, Friday"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_card_follows_the_editor_undo_and_redo_and_the_gaps_left() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    let serial = drafted_into(&app, &reply).await;

    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Written, true, vec![1], 2),
        0,
    );
    let ReplyDraftState::Drafted { checklist, .. } = view(&app, &reply).state else {
        panic!("the card");
    };
    assert_eq!([checklist[0].done, checklist[1].done], [true, false]);

    // Undone: the card goes; redone: it is back, with the gap the undo restored open again.
    app.take_reply_draft_report(&reply, report(serial, LeadText::Empty, false, vec![], 3), 0);
    assert_eq!(view(&app, &reply).state, ReplyDraftState::Ready);
    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Draft, true, vec![0, 1], 4),
        0,
    );
    let ReplyDraftState::Drafted { checklist, .. } = view(&app, &reply).state else {
        panic!("the card again");
    };
    assert!(!checklist[0].done);
    assert!(
        app.composition_drafted(&reply),
        "drafted for good once it went in"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_person_ticks_what_the_reply_cannot_show_and_never_a_gap() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    drafted_into(&app, &reply).await;

    app.tick_reply_draft_item(&reply, 0, true);
    app.tick_reply_draft_item(&reply, 3, true);
    let ReplyDraftState::Drafted { checklist, .. } = view(&app, &reply).state else {
        panic!("the card");
    };
    assert_eq!(
        checklist.iter().map(|item| item.done).collect::<Vec<_>>(),
        [false, false, false, true]
    );
    // An attach item ticks once a file is added after the draft.
    let attached = app.composer_reply_draft(&reply, None, &|_| 1).unwrap();
    let ReplyDraftState::Drafted { checklist, .. } = attached.state else {
        panic!("the card");
    };
    assert!(checklist[2].done);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replacing_the_persons_text_is_asked_first_and_again_when_they_wrote_meanwhile() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;

    // Until the editor has said, it asks.
    assert!(view(&app, &reply).ask_before_replacing);
    app.take_reply_draft_report(&reply, report(0, LeadText::Written, false, vec![], 5), 0);
    assert!(view(&app, &reply).ask_before_replacing);
    assert_eq!(
        app.draft_reply(&reply, start(false)),
        None,
        "not without asking"
    );
    assert_eq!(view(&app, &reply).state, ReplyDraftState::Ready);

    // Asked and answered, untouched while it drafts: it goes in without a second question.
    assert_eq!(app.draft_reply(&reply, start(true)), Some(AiQueued::Run));
    run(&app).await;
    let untouched = view(&app, &reply);
    assert!(!untouched.replace_arrived);
    assert!(untouched.editor_script.is_some());
    app.stop_reply_draft(&reply);

    // Written meanwhile: the draft waits for the question, and Cancel keeps the person's text.
    assert_eq!(app.draft_reply(&reply, start(true)), Some(AiQueued::Run));
    app.take_reply_draft_report(&reply, report(0, LeadText::Written, false, vec![], 6), 0);
    run(&app).await;
    let arrived = view(&app, &reply);
    assert!(arrived.replace_arrived);
    assert_eq!(arrived.state, ReplyDraftState::Drafting);
    assert!(arrived.editor_script.is_none());
    app.answer_reply_draft_arrived(&reply, false);
    let kept = view(&app, &reply);
    assert!(!kept.replace_arrived && kept.editor_script.is_none());
    assert_eq!(kept.state, ReplyDraftState::Ready);

    // And Replace hands it to the editor.
    assert_eq!(app.draft_reply(&reply, start(true)), Some(AiQueued::Run));
    app.take_reply_draft_report(&reply, report(0, LeadText::Written, false, vec![], 7), 0);
    run(&app).await;
    app.answer_reply_draft_arrived(&reply, true);
    assert!(view(&app, &reply).editor_script.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_abandons_the_draft_and_its_answer_is_dropped() {
    let (relay, release) = ScriptedRelay::held(Ok(drafted()));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    app.take_reply_draft_report(&reply, report(0, LeadText::Empty, false, vec![], 0), 0);
    assert_eq!(app.draft_reply(&reply, start(false)), Some(AiQueued::Run));
    let runner = tokio::task::spawn_blocking({
        let app = Arc::clone(&app);
        move || app.run_ai_work()
    });
    while relay.seen.lock().unwrap().is_empty() {
        tokio::task::yield_now().await;
    }

    app.stop_reply_draft(&reply);
    let stopped = view(&app, &reply);
    assert_eq!(
        stopped.state,
        ReplyDraftState::Ready,
        "Stop is not a failure"
    );
    assert!(!stopped.stop.enabled);
    release.send(()).unwrap();
    runner.await.unwrap();
    assert!(
        view(&app, &reply).editor_script.is_none(),
        "the late answer is dropped"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn what_the_relay_refused_is_a_failure_with_its_remedy() {
    let relay = ScriptedRelay::answering(Err(AiError::Unauthorized));
    let (app, _) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    app.take_reply_draft_report(&reply, report(0, LeadText::Empty, false, vec![], 0), 0);
    app.draft_reply(&reply, start(false));
    run(&app).await;
    let failed = view(&app, &reply);
    assert_eq!(
        failed.state,
        ReplyDraftState::Failed {
            failure: AiFailure::RelaySignIn,
            remedy: Some(AiRemedy::SignInToAllodia),
        }
    );
    assert!(failed.draft.enabled, "it can be asked again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn it_says_why_it_cannot_draft_and_is_offered_on_a_reply_only() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let reply = open_reply(&app, "reply").await;
    assert!(
        app.composer_reply_draft(&reply, None, &|_| 0).is_none(),
        "not offered"
    );
    app.install_ai_host(
        Arc::new(RuntimeHost(tokio::runtime::Handle::current())),
        BuildKind::Development,
    );
    let unavailable = |app: &App<ThreadProvider>| match view(app, &reply).state {
        ReplyDraftState::Unavailable { failure, remedy } => (failure, remedy),
        other => panic!("unavailable, not {other:?}"),
    };
    assert_eq!(
        unavailable(&app),
        (AiFailure::NoStyle, Some(AiRemedy::LearnStyle))
    );
    let control = view(&app, &reply);
    assert!(
        control.control.enabled,
        "pressed, it opens the input with the reason"
    );
    assert!(!control.draft.enabled);
    assert_eq!(control.held_by, Some(AiFailure::NoStyle));

    learn_a_style(&app);
    app.assign_writing_style("acct-1", None);
    assert_eq!(
        unavailable(&app),
        (AiFailure::NoStyleForAccount, Some(AiRemedy::ChooseStyle))
    );
    learn_a_style(&app);
    let first = app.writing_styles()[0].0.clone();
    app.assign_writing_style("acct-1", Some(first));
    assert_eq!(
        unavailable(&app),
        (AiFailure::NowhereToSend, Some(AiRemedy::OpenAiEndpoint))
    );

    let scratch = tempfile::tempdir().unwrap();
    let new = CompositionId::new("new").unwrap();
    app.open_composer(
        new.clone(),
        ComposeOrigin::New { account: None },
        None,
        &staging_dir(&scratch, "new"),
    )
    .await
    .unwrap();
    assert!(app.composer_reply_draft(&new, None, &|_| 0).is_none());
}
