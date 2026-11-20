//! The writing assistant in a production build whose entitlement does not carry it (`docs/ai.md`
//! rules 4 and 5), walked across every accessor and intent a client reaches it by: each record is
//! absent and each intent changes nothing, so what the person finds once the offer is granted is
//! what was there before.

use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

use mailcal_app::ai::AiEntitlement;

use crate::{
    AiEndpointForm, AiEndpointIntent, AiIntent, ComposeOrigin, ComposerReplyDraft, Intent,
    JurisdictionClass, LogLevel, MailcalApp, ReplyDraftIntent, ShowcaseLocale, SnapshotRow,
    StyleLearnRange, StyleLearningIntent, WritingStyleIntent, WritingStylePage,
    tests::{ChannelObserver, NullLogger},
};

/// The showcase with its styles, its endpoint and an open reply, as a production build holds it
/// with the entitlement granted; its first account's id and its styles' ids.
fn staged() -> (Arc<MailcalApp>, String, Vec<String>) {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );
    let account = app.app.account_ids()[0].as_str().to_owned();
    super::super::offer_ai(&app);
    super::stage(app.app.as_ref(), &account, &account);
    super::super::offer_ai_in_production(&app);
    app.app.set_ai_entitlement(AiEntitlement::Granted);
    open_reply(&app);
    let styles = app
        .writing_style_page()
        .expect("offered")
        .styles
        .into_iter()
        .map(|style| style.id)
        .collect();
    (app, account, styles)
}

fn open_reply(app: &MailcalApp) {
    let (account, key) = app
        .mailbox_list()
        .rows
        .into_iter()
        .map(|row| match row {
            SnapshotRow::Flat { row } => (row.account, row.key),
            SnapshotRow::Thread { row } => (row.account, row.latest_key),
        })
        .next()
        .expect("the inbox is seeded");
    let staging = crate::tests::temp_data_dir("not-offered-reply");
    app.open_composer(
        "reply".to_owned(),
        ComposeOrigin::Reply {
            account,
            key,
            all: false,
        },
        staging.to_string_lossy().into_owned(),
        None,
    )
    .expect("the reply opens");
}

/// What every surface of the feature draws.
#[derive(Debug, PartialEq)]
struct Drawn {
    page: Option<WritingStylePage>,
    endpoint: Option<AiEndpointForm>,
    learning: bool,
    reply: Option<ComposerReplyDraft>,
}

fn drawn(app: &MailcalApp) -> Drawn {
    Drawn {
        page: app.writing_style_page(),
        endpoint: app.ai_endpoint_form(),
        learning: app.writing_style_learning().is_some(),
        reply: app.composer_reply_draft("reply".to_owned(), None),
    }
}

#[test]
fn not_offered_every_record_is_absent() {
    let (app, _, styles) = staged();
    app.app.set_ai_entitlement(AiEntitlement::NotGranted);
    assert!(!app.ai_offer());
    let mut records = vec![
        ("ai_endpoint_form", app.ai_endpoint_form().is_some()),
        ("writing_style_page", app.writing_style_page().is_some()),
        (
            "writing_style_learning",
            app.writing_style_learning().is_some(),
        ),
        (
            "composer_reply_draft",
            app.composer_reply_draft("reply".to_owned(), None).is_some(),
        ),
    ];
    for style in &styles {
        records.push((
            "writing_style_reveal",
            app.writing_style_reveal(style.clone()).is_some(),
        ));
        records.push((
            "writing_style_detail",
            app.writing_style_detail(style.clone()).is_some(),
        ));
    }
    for (record, present) in records {
        assert!(!present, "{record} is drawn while not offered");
    }
}

#[test]
fn not_offered_every_intent_changes_nothing() {
    let (app, account, styles) = staged();
    let before = drawn(&app);
    assert!(before.page.is_some() && before.endpoint.is_some() && before.reply.is_some());

    app.app.set_ai_entitlement(AiEntitlement::NotGranted);
    let learning = [
        StyleLearningIntent::Open,
        StyleLearningIntent::SelectAccount {
            account: account.clone(),
        },
        StyleLearningIntent::SelectRange {
            range: StyleLearnRange::Everything,
        },
        StyleLearningIntent::Next,
        StyleLearningIntent::Learn {
            name: "Mine".to_owned(),
            language: "en".to_owned(),
        },
        StyleLearningIntent::Back,
        StyleLearningIntent::Stop,
        StyleLearningIntent::Remedy,
        StyleLearningIntent::Close,
    ]
    .map(|intent| WritingStyleIntent::Learning { intent });
    let mut intents: Vec<AiIntent> = learning
        .into_iter()
        .map(|intent| AiIntent::WritingStyle { intent })
        .collect();
    for style in &styles {
        intents.extend(
            [
                WritingStyleIntent::Rename {
                    style: style.clone(),
                    name: "Renamed".to_owned(),
                },
                WritingStyleIntent::SetNotes {
                    style: style.clone(),
                    notes: "Changed.".to_owned(),
                },
                WritingStyleIntent::SaveReveal {
                    style: style.clone(),
                    name: "Saved".to_owned(),
                    notes: String::new(),
                },
                WritingStyleIntent::CloseReveal {
                    style: style.clone(),
                },
                WritingStyleIntent::Forget {
                    style: style.clone(),
                },
            ]
            .map(|intent| AiIntent::WritingStyle { intent }),
        );
    }
    intents.extend([
        AiIntent::WritingStyle {
            intent: WritingStyleIntent::Assign {
                account: account.clone(),
                style: None,
            },
        },
        AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: "https://models.example.eu/v1".to_owned(),
                model: "other".to_owned(),
                declared: Some(JurisdictionClass::EuNative),
                key: "sk-test".to_owned(),
            },
        },
        AiIntent::Endpoint {
            intent: AiEndpointIntent::Remove,
        },
        AiIntent::ReplyDraft {
            intent: ReplyDraftIntent::Draft {
                composition: "reply".to_owned(),
                from: None,
                instruction: "yes".to_owned(),
                language: "en".to_owned(),
                replace: true,
            },
        },
    ]);
    for intent in intents {
        app.dispatch(Intent::Ai { intent });
    }
    // A change the core let through would run on a thread of its own: give it the time to land.
    std::thread::sleep(Duration::from_millis(200));

    app.app.set_ai_entitlement(AiEntitlement::Granted);
    assert_eq!(drawn(&app), before);
}
