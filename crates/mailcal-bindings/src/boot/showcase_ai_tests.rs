//! What `MAILCAL_OFFER_AI` stages for the UI suites: a row that opens the reveal and one that opens
//! the detail, every page of the reveal in two languages, and a reply on the showcase that drafts
//! from a canned answer, or fails as unreachable once the endpoint asks another model.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use mailcal_ai::HttpRequest;

use crate::{
    AiEndpointIntent, AiFailure, AiIntent, ComposeOrigin, ComposerReplyDraft, Intent,
    JurisdictionClass, LogLevel, MailcalApp, ReplyDraftIntent, ReplyDraftState, ReplyDraftTaskKind,
    ShowcaseLocale, SnapshotRow, WritingStyleOpens, WritingStyleRevealPage,
    tests::{ChannelObserver, NullLogger},
};

fn showcase() -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    )
}

/// The showcase as the hook leaves it, and its first account's id.
fn staged() -> (Arc<MailcalApp>, String) {
    let app = showcase();
    let account = app.app.account_ids()[0].as_str().to_owned();
    super::super::offer_ai(&app);
    super::stage(app.app.as_ref(), &account, &account);
    (app, account)
}

fn eventually<T>(mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "never came");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn one_row_opens_the_detail_and_one_the_reveal_with_every_page_in_two_languages() {
    let (app, account) = staged();
    let page = app.writing_style_page().expect("offered");
    let opens: Vec<_> = page.styles.iter().map(|style| style.opens).collect();
    assert_eq!(
        opens,
        [WritingStyleOpens::Detail, WritingStyleOpens::Reveal]
    );
    let slot = page
        .accounts
        .iter()
        .find(|slot| slot.account_id == account)
        .expect("the account has a slot");
    assert_eq!(
        slot.selected.as_deref(),
        Some(page.styles[0].id.as_str()),
        "the account drafts in the style whose detail opens"
    );
    assert!(
        app.writing_style_detail(page.styles[0].id.clone())
            .is_some()
    );

    let reveal = app
        .writing_style_reveal(page.styles[1].id.clone())
        .expect("the reveal");
    assert_eq!(
        reveal.pages,
        [
            WritingStyleRevealPage::Read,
            WritingStyleRevealPage::Letter,
            WritingStyleRevealPage::Habits,
            WritingStyleRevealPage::Voice,
            WritingStyleRevealPage::Phrases,
            WritingStyleRevealPage::Name,
        ]
    );
    for code in ["en", "nl"] {
        let language = reveal
            .languages
            .iter()
            .find(|language| language.language == code)
            .unwrap_or_else(|| panic!("{code} is learned"));
        assert!(language.letter.is_some(), "{code}");
        assert!(language.habits.is_some(), "{code}");
        assert!(language.cards.len() >= 3, "{code}");
        assert!(language.phrases.is_some(), "{code}");
    }
    let german = reveal
        .languages
        .iter()
        .find(|language| language.language == "de")
        .expect("a language with gaps");
    assert!(german.cards.is_empty() && german.phrases.is_none());
}

/// Draft a reply on the showcase reaches every state a suite draws: offered, drafted from the
/// canned answer with a summary and an item of each kind, and failed as unreachable.
#[test]
fn a_reply_drafts_from_the_canned_answer_and_fails_as_unreachable_on_another_model() {
    let (app, _) = staged();
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
    let staging = crate::tests::temp_data_dir("showcase-ai-draft");
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
    let record = || app.composer_reply_draft("reply".to_owned(), None);
    assert_eq!(record().expect("offered").state, ReplyDraftState::Ready);
    let report = |serial: u64, lead: &str, inserted: bool| {
        let left = if inserted { "[0]" } else { "[]" };
        let message = format!(
            r#"{{"id":0,"request":{{"reply_draft":{{"serial":{serial},"lead":"{lead}","inserted":{inserted},"left":{left},"edits":0}}}}}}"#
        );
        assert!(app.composer_reply_draft_report("reply".to_owned(), message));
    };
    report(0, "empty", false);
    let draft = || {
        app.dispatch(Intent::Ai {
            intent: AiIntent::ReplyDraft {
                intent: ReplyDraftIntent::Draft {
                    composition: "reply".to_owned(),
                    from: None,
                    instruction: "yes".to_owned(),
                    language: "en".to_owned(),
                    replace: true,
                },
            },
        });
    };

    draft();
    let script = eventually(|| record().and_then(|record| record.editor_script));
    report(script.serial, "draft", true);
    let drafted: ComposerReplyDraft = eventually(|| {
        record().filter(|record| matches!(record.state, ReplyDraftState::Drafted { .. }))
    });
    let ReplyDraftState::Drafted { summary, checklist } = drafted.state else {
        unreachable!()
    };
    assert!(!summary.is_empty());
    let kinds: Vec<_> = checklist.iter().map(|item| item.kind).collect();
    assert_eq!(
        kinds,
        [
            ReplyDraftTaskKind::FillIn,
            ReplyDraftTaskKind::Attach,
            ReplyDraftTaskKind::Do
        ]
    );

    let endpoint = app.ai_endpoint_form().expect("offered");
    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: endpoint.address,
                model: "another-model".to_owned(),
                declared: Some(JurisdictionClass::EuNative),
                key: String::new(),
            },
        },
    });
    eventually(|| {
        app.ai_endpoint_form()
            .filter(|form| form.outcome.is_some() && !form.save.running)
    });
    draft();
    let failed = eventually(|| {
        record().filter(|record| matches!(record.state, ReplyDraftState::Failed { .. }))
    });
    assert!(matches!(
        failed.state,
        ReplyDraftState::Failed {
            failure: AiFailure::Unreachable,
            ..
        }
    ));
}

#[test]
fn the_canned_answer_is_for_a_draft_to_the_staged_model_alone() {
    let draft_body = |model: &str| {
        serde_json::json!({
            "model": model,
            "tools": [{ "function": { "parameters": { "properties": { "reply": {} } } } }],
        })
        .to_string()
    };
    let post = |body: &str| {
        super::transport().post_json(HttpRequest {
            url: "http://localhost:11434/v1/chat/completions",
            bearer: None,
            body,
            timeout: Duration::from_secs(1),
        })
    };
    assert!(post(&draft_body(super::CANNED_MODEL)).is_ok_and(|answer| answer.status == 200));
    assert!(post(&draft_body("another-model")).is_err());
    let learning = serde_json::json!({ "model": super::CANNED_MODEL }).to_string();
    assert!(post(&learning).is_err(), "learning answers as no answer");
}
