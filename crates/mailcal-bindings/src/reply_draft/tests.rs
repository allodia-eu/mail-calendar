//! Draft a reply as a host reaches it: the editor's report taken by the core and by nothing else,
//! the record as a client draws it, and intents for a composition that is not a reply changing
//! nothing. The flow itself is held by `mailcal-app`'s tests.

use std::sync::{Arc, mpsc};

use mailcal_app::ai::{
    AiFailure as CoreFailure, REPLY_DRAFT_SUGGESTIONS, ReplyDraftControl,
    ReplyDraftItem as CoreItem, ReplyDraftScript as CoreScript, ReplyDraftState as CoreState,
    ReplyDraftView,
};

use super::{
    ComposerReplyDraft, ReplyDraftHold, ReplyDraftIntent, ReplyDraftSendQuestion, ReplyDraftState,
    ReplyDraftSuggestion, ReplyDraftTaskKind,
};
use crate::{
    AiFailure, AiIntent, AiRemedy, ComposeOrigin, ComposerHostRequest, Intent, LogLevel,
    MailcalApp,
    composer_host::parse_composer_host_request,
    tests::{ChannelObserver, NullLogger},
};

const REPORT: &str = r#"{"id":0,"request":{"reply_draft":{"serial":0,"lead":"empty","inserted":false,"left":[],"edits":0}}}"#;

fn app() -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    app.install_ai(None);
    app
}

#[test]
fn the_editors_report_is_the_cores_and_never_a_clients_request() {
    let app = app();
    assert_eq!(parse_composer_host_request(REPORT.to_owned()), None);
    assert!(app.composer_reply_draft_report("c1".to_owned(), REPORT.to_owned()));
    let link = r#"{"id":2,"request":{"link":{"text":"docs","address":"","removable":false}}}"#;
    assert!(!app.composer_reply_draft_report("c1".to_owned(), link.to_owned()));
    assert!(matches!(
        parse_composer_host_request(link.to_owned()),
        Some(ComposerHostRequest::Link { .. })
    ));
    assert!(!app.composer_reply_draft_report("c1".to_owned(), "garbage".to_owned()));
}

#[test]
fn a_new_message_offers_no_draft_and_its_intents_change_nothing() {
    let app = app();
    assert!(app.ai_offer(), "a development build offers it");
    app.open_composer(
        "new".to_owned(),
        ComposeOrigin::New { account: None },
        crate::tests::temp_data_dir("reply-draft")
            .to_string_lossy()
            .into_owned(),
        None,
    )
    .expect("the composer opens");
    assert_eq!(app.composer_reply_draft("new".to_owned(), None), None);
    assert_eq!(app.composer_reply_draft(String::new(), None), None);
    for intent in [
        ReplyDraftIntent::Draft {
            composition: "new".to_owned(),
            from: None,
            instruction: "yes".to_owned(),
            language: "en".to_owned(),
            replace: true,
        },
        ReplyDraftIntent::Stop {
            composition: "new".to_owned(),
        },
        ReplyDraftIntent::AnswerArrived {
            composition: "new".to_owned(),
            replace: true,
        },
        ReplyDraftIntent::Tick {
            composition: "new".to_owned(),
            index: 0,
            done: true,
        },
        ReplyDraftIntent::SendQuestionAnswered {
            composition: String::new(),
        },
    ] {
        app.dispatch(Intent::Ai {
            intent: AiIntent::ReplyDraft { intent },
        });
    }
    assert_eq!(app.composer_reply_draft("new".to_owned(), None), None);
    assert!(!app.reply_draft_send_due(&mailcal_app::CompositionId::new("new").unwrap()));
}

#[test]
fn the_record_numbers_its_items_and_names_each_hold_with_its_remedy() {
    const ON: ReplyDraftControl = ReplyDraftControl {
        enabled: true,
        running: false,
    };
    let item = |kind, done| CoreItem {
        kind,
        text: "[date]".to_owned(),
        done,
        tick: ON,
    };
    let view = ReplyDraftView {
        state: CoreState::Drafted {
            summary: "Anna asks.".to_owned(),
            checklist: vec![
                item(mailcal_ai::TaskKind::FillIn, true),
                item(mailcal_ai::TaskKind::Do, false),
            ],
        },
        control: ON,
        draft: ReplyDraftControl {
            enabled: false,
            running: false,
        },
        stop: ReplyDraftControl {
            enabled: false,
            running: false,
        },
        held_by: Some(CoreFailure::NoStyleForAccount),
        suggestions: REPLY_DRAFT_SUGGESTIONS.to_vec(),
        instruction: "yes".to_owned(),
        ask_before_replacing: true,
        replace_arrived: false,
        editor_script: Some(CoreScript {
            serial: 3,
            script: "window.setComposerDraftText({});".to_owned(),
        }),
        send_question: Some(1),
    };
    let record = ComposerReplyDraft::from(view);
    let ReplyDraftState::Drafted { checklist, .. } = &record.state else {
        panic!("the card");
    };
    assert_eq!(
        checklist
            .iter()
            .map(|item| (item.index, item.kind, item.done))
            .collect::<Vec<_>>(),
        [
            (0, ReplyDraftTaskKind::FillIn, true),
            (1, ReplyDraftTaskKind::Do, false)
        ]
    );
    assert_eq!(
        record.held_by,
        Some(ReplyDraftHold {
            failure: AiFailure::NoStyleForAccount,
            remedy: Some(AiRemedy::ChooseStyle),
        })
    );
    assert_eq!(
        record.suggestions,
        [
            ReplyDraftSuggestion::Yes,
            ReplyDraftSuggestion::No,
            ReplyDraftSuggestion::MoreInformation,
            ReplyDraftSuggestion::Holding
        ]
    );
    assert_eq!(
        record.send_question,
        Some(ReplyDraftSendQuestion { open: 1 })
    );
    assert_eq!(record.editor_script.map(|script| script.serial), Some(3));
}
