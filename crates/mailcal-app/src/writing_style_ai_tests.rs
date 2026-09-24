//! Learning and drafting, end to end through the app: sent mail read from the store, cut to the
//! author's words on the device, sent through the gated backend to an OpenAI-compatible endpoint,
//! and the answer stored or returned. The endpoint is a canned one behind the transport port, so
//! everything above the socket is the code that ships.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, EmailAddress, UtcDateTime};
use mailcal_ai::{
    AiError, Class, GatedBackend, HttpRequest, HttpResponse, HttpTransport, OwnEndpoint, TaskKind,
    TransportFailed,
};
use serde_json::{Value, json};

use super::fakes::{FakeProvider, account, app, message, msg};
use crate::{App, Intent, LearnRange, ReplyDraftRequest, Surface, WritingStyleError};

/// The one source the fake provider serves for every message: the author's own words, their
/// sign-off, a signature block, and a quoted original below it.
const SOURCE: &[u8] = b"Content-Type: text/plain; charset=utf-8\r\n\r\n\
Thanks for sending the figures over. I have had a look at them this morning and they are \
mostly what we expected, although the travel costs are higher than last quarter. Could you \
check whether the hotel in Lyon was booked twice? If so, let us ask for a refund before the end \
of the month.\r\n\r\nBest,\r\nSam\r\n-- \r\nSam Jansen | Finance\r\n\r\n\
On 1 Jul 2026, Anna <anna@example.eu> wrote:\r\n> The quoted question about the budget.\r\n";

/// An OpenAI-compatible endpoint that records every body and answers each request through its
/// forced tool: a draft when the tool's schema asks for a reply, a style otherwise.
struct FakeEndpoint {
    seen: Arc<Mutex<Vec<Value>>>,
}

impl HttpTransport for FakeEndpoint {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        let body: Value = serde_json::from_str(request.body).unwrap();
        let wants_reply = body["tools"][0]["function"]["parameters"]["properties"]
            .get("reply")
            .is_some();
        let answer = if wants_reply {
            let drafted = json!({
                "summary": "Anna asks whether Friday suits.",
                "reply": "Hi Anna,\n\nFine by me on **[date]**.\n\nBest,\nSam",
                "tasks": [{ "kind": "attach", "text": "Attach the figures" }],
            });
            json!({ "choices": [{ "message": { "tool_calls": [{ "function": {
                "name": "emit_json", "arguments": drafted.to_string() } }] } }] })
        } else {
            let described = json!({
                "style": { "greetings": [{ "text": "Hi", "share": 80 }], "typical_words": 60,
                           "register": "Direct, first names." },
                "exemplars": [1],
            });
            json!({ "choices": [{ "message": { "tool_calls": [{ "function": {
                "name": "emit_json", "arguments": described.to_string() } }] } }] })
        };
        self.seen.lock().unwrap().push(body);
        Ok(HttpResponse {
            status: 200,
            body: answer.to_string(),
        })
    }
}

fn at(rfc3339: &str) -> UtcDateTime {
    UtcDateTime::parse_rfc3339(rfc3339).unwrap()
}

fn sent(key: &str, date: &str) -> engine_api::Message {
    let mut sent = message(key, "sent", "Re: figures");
    sent.sent_at = Some(at(date));
    sent.envelope.message_id =
        vec![engine_api::MessageIdHeader::new(format!("{key}@example.eu")).unwrap()];
    sent.envelope.to = vec![EmailAddress::new("anna@example.eu")];
    sent
}

/// An account with three sent messages (2024, 2025, 2026) and one received from Anna. The app is
/// boxed so the test futures holding it stay small.
async fn fixture() -> (Box<App<FakeProvider>>, Arc<Mutex<Vec<Surface>>>) {
    let mut received = message("in-1", "a", "Budget");
    received.envelope.from = vec![EmailAddress::named("Anna", "anna@example.eu")];
    received.received_at = Some(at("2026-07-02T09:00:00Z"));
    let provider = FakeProvider::with_sent_and_archive(vec![
        sent("s-2024", "2024-06-01T10:00:00Z"),
        sent("s-2025", "2025-06-01T10:00:00Z"),
        sent("s-2026", "2026-06-01T10:00:00Z"),
        received,
    ])
    .with_source(SOURCE);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = Box::new(app(vec![account("acct-1", provider)], &surfaces));
    app.dispatch(Intent::RefreshMail).await;
    (app, surfaces)
}

fn install(app: &App<FakeProvider>, declared: Class) -> Arc<Mutex<Vec<Value>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    app.set_ai_backend(Some(GatedBackend::own_endpoint(
        OwnEndpoint::new(
            "http://localhost:11434/v1",
            None,
            "mistral-small",
            Some(declared),
        )
        .unwrap(),
        Box::new(FakeEndpoint {
            seen: Arc::clone(&seen),
        }),
        app.jurisdiction_mode_source(),
    )));
    seen
}

fn account_id() -> AccountId {
    AccountId::try_from("acct-1").unwrap()
}

#[tokio::test]
async fn the_report_reads_only_the_sent_folder_and_only_the_range() {
    let (app, _) = fixture().await;
    let everything = app
        .sent_corpus_report(&account_id(), LearnRange::default())
        .await
        .unwrap();
    assert_eq!(everything.found, 3);
    // One source served for all three, so they are one message sent three times.
    assert_eq!(everything.usable, 1);
    assert_eq!(everything.languages[0].language, "en");

    let until_2025 = app
        .sent_corpus_report(
            &account_id(),
            LearnRange {
                since: None,
                until: Some(1_767_225_599), // 2025-12-31T23:59:59Z
            },
        )
        .await
        .unwrap();
    assert_eq!(until_2025.found, 2);
    // The horizon is the oldest sent message on the device, whatever the range.
    assert_eq!(until_2025.horizon, Some(1_717_236_000)); // 2024-06-01T10:00:00Z
}

#[tokio::test]
async fn learning_sends_only_the_author_s_words_and_stores_an_assigned_style() {
    let (app, surfaces) = fixture().await;
    let seen = install(&app, Class::EuNative);

    let report = app
        .learn_writing_style(
            &account_id(),
            LearnRange::default(),
            "Work".to_owned(),
            "nl",
        )
        .await
        .unwrap();

    assert_eq!(report.languages, ["en"]);
    assert_eq!(report.messages, 1);
    assert!(report.metering.is_none(), "an own endpoint charges nothing");
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["model"], "mistral-small");
    assert_eq!(seen[0]["tool_choice"]["function"]["name"], "emit_json");
    let mail = seen[0]["messages"][1]["content"].as_str().unwrap();
    assert!(mail.contains("hotel in Lyon"));
    assert!(
        !mail.contains("quoted question"),
        "the quote left the device"
    );
    assert!(
        !mail.contains("Sam Jansen"),
        "the signature left the device"
    );
    assert!(
        seen[0]["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("in Dutch")
    );

    assert_eq!(
        app.resolve_writing_style("acct-1").as_deref(),
        Some(report.style_id.as_str())
    );
    let snapshot = app.writing_styles().await;
    assert!(snapshot.learning.is_none(), "the run is over");
    assert_eq!(snapshot.styles[0].name, "Work");
    assert!(surfaces.lock().unwrap().contains(&Surface::WritingStyle));
}

#[tokio::test]
async fn a_draft_answers_the_message_in_the_account_s_style() {
    let (app, _) = fixture().await;
    let seen = install(&app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();

    let draft = app
        .draft_reply(&ReplyDraftRequest {
            message: msg("acct-1", "in-1"),
            from: None,
            style: None,
            intent: Some("yes, but next week".to_owned()),
            language: None,
            ui_language: "en".to_owned(),
        })
        .await
        .unwrap();

    assert!(draft.text.starts_with("Hi Anna"));
    assert_eq!(draft.gaps, ["[date]"]);
    assert_eq!(draft.language, "en");
    let seen = seen.lock().unwrap();
    let material = seen[1]["messages"][1]["content"].as_str().unwrap();
    // The passage the learn picked, the last message sent to Anna, the thread and the intent.
    assert!(material.contains("Passages this person wrote"));
    assert!(material.contains("recently wrote to the same recipient"));
    assert!(material.contains("<untrusted-message-content from=Anna anna@example.eu"));
    assert!(material.contains("yes, but next week"));
    // The draft comes with what the message asks and a checklist: the reply's gap first.
    assert_eq!(draft.summary, "Anna asks whether Friday suits.");
    let kinds: Vec<_> = draft.tasks.iter().map(|task| task.kind).collect();
    assert_eq!(kinds, [TaskKind::FillIn, TaskKind::Attach]);
    assert_eq!(draft.tasks[0].text, "[date]");
}

#[tokio::test]
async fn a_refused_destination_sends_nothing_and_says_why() {
    let (app, _) = fixture().await;
    let seen = install(&app, Class::NonEu);

    let snapshot = app.writing_styles().await;
    let refused = snapshot.refused.expect("the gate would refuse");
    assert_eq!(refused.class, Class::NonEu);

    let failure = app
        .learn_writing_style(
            &account_id(),
            LearnRange::default(),
            "Work".to_owned(),
            "en",
        )
        .await
        .unwrap_err();
    assert!(matches!(
        failure.error,
        WritingStyleError::Ai(AiError::Refused(_))
    ));
    assert!(seen.lock().unwrap().is_empty());
    assert!(app.writing_styles().await.learning.is_none());
    assert!(app.writing_styles().await.styles.is_empty());
}

#[tokio::test]
async fn without_a_backend_nothing_is_read_or_sent() {
    let (app, _) = fixture().await;
    let failure = app
        .learn_writing_style(
            &account_id(),
            LearnRange::default(),
            "Work".to_owned(),
            "en",
        )
        .await
        .unwrap_err();
    assert_eq!(failure.error, WritingStyleError::Unavailable);
    let draft = app
        .draft_reply(&ReplyDraftRequest {
            message: msg("acct-1", "in-1"),
            from: None,
            style: None,
            intent: None,
            language: None,
            ui_language: "en".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(draft, WritingStyleError::Unavailable);
    assert!(!app.ai_available());
}

#[tokio::test]
async fn a_draft_with_no_style_to_write_in_is_refused_before_anything_is_sent() {
    let (app, _) = fixture().await;
    let seen = install(&app, Class::EuNative);
    let draft = app
        .draft_reply(&ReplyDraftRequest {
            message: msg("acct-1", "in-1"),
            from: None,
            style: None,
            intent: None,
            language: None,
            ui_language: "en".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(draft, WritingStyleError::NoStyle);
    assert!(seen.lock().unwrap().is_empty());
}

/// A reply sent from a draft is logged with what the person changed, and from then on it is left
/// out of learning: a style learned from a model's words would drift towards the model.
#[tokio::test]
async fn a_reply_sent_from_a_draft_is_logged_and_never_learned_from() {
    let (app, _) = fixture().await;
    install(&app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();
    let draft = app
        .draft_reply(&ReplyDraftRequest {
            message: msg("acct-1", "in-1"),
            from: None,
            style: None,
            intent: None,
            language: None,
            ui_language: "en".to_owned(),
        })
        .await
        .unwrap();

    let document: mailcal_composer::ComposerDocument = serde_json::from_value(json!({
        "blocks": [
            { "Paragraph": { "content": [{ "Text": { "text": "Hi Anna," } }] } },
            { "Paragraph": { "content": [{ "Text": { "text": "Fine by me on Friday." } }] } },
            { "Paragraph": { "content": [{ "Text": { "text": "Best, Sam" } }] } },
        ],
    }))
    .unwrap();
    app.dispatch(Intent::SubmitRichReply {
        message: msg("acct-1", "in-1"),
        from: None,
        to: "anna@example.eu".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs: Vec::new(),
        composition: None,
        ai_draft: Some(draft.draft_id.clone()),
    })
    .await;

    let sends = app.writing_style.observed.sends();
    assert_eq!(sends.len(), 1);
    let send = &sends[0];
    assert_eq!(send.account, "acct-1");
    assert_eq!(send.language, "en");
    // The person replaced the gap with a day: that is the whole correction. The bold around the
    // gap was the composer's to draw, so it is not an edit.
    assert_eq!(send.added, ["Friday."]);
    assert_eq!(send.removed, ["[date]."]);
}

async fn found_in(app: &App<FakeProvider>) -> u32 {
    app.sent_corpus_report(&account_id(), LearnRange::default())
        .await
        .unwrap()
        .found
}

/// Once a sent message is logged as written from a draft, learning passes over it.
#[tokio::test]
async fn learning_passes_over_a_message_sent_from_a_draft() {
    let (app, _) = fixture().await;
    assert_eq!(found_in(&app).await, 3);

    app.writing_style.observed.edit(|log| {
        log.record(mailcal_account::AiAssistedSend {
            message_id: "s-2026@example.eu".to_owned(),
            account: "acct-1".to_owned(),
            style: "work".to_owned(),
            ..mailcal_account::AiAssistedSend::default()
        });
    });

    assert_eq!(found_in(&app).await, 2);
}

/// A draft id the session never issued (one from before a restart, or made up) logs nothing.
#[tokio::test]
async fn an_unknown_draft_id_logs_nothing() {
    let (app, _) = fixture().await;
    app.note_ai_draft_sent("acct-1", "never-issued", "Hello", "x@y");
    assert!(app.writing_style.observed.sends().is_empty());
}

/// A style from another device arrives with no passages. This device picks its own from its own
/// sent mail, cut to the author's words, with no backend installed at all.
#[tokio::test]
async fn a_style_from_another_device_takes_its_passages_from_this_device_s_sent_mail() {
    let (app, surfaces) = fixture().await;
    let mut guide = mailcal_ai::StyleGuide::new();
    for language in ["en", "de"] {
        guide.languages.insert(
            language.to_owned(),
            mailcal_ai::LanguageStyle {
                typical_words: 60,
                ..mailcal_ai::LanguageStyle::default()
            },
        );
    }
    guide.notes = "Short.".to_owned();

    let id = app
        .add_synced_writing_style("Work".to_owned(), &guide)
        .await;

    let library = app.writing_style.library();
    let style = library
        .get(&mailcal_account::WritingStyleId::new(id).unwrap())
        .unwrap();
    assert_eq!(style.name, "Work");
    assert!(style.source.is_empty(), "no account here is its source");
    let stored: mailcal_ai::StyleGuide = serde_json::from_str(&style.guide_json).unwrap();
    assert_eq!(stored, guide, "the guide is kept as it arrived");
    let passages: mailcal_ai::Exemplars = serde_json::from_str(&style.exemplars_json).unwrap();
    assert_eq!(
        passages.languages.keys().collect::<Vec<_>>(),
        ["en"],
        "no German mail on this device, so no German passages"
    );
    let passage = &passages.languages["en"][0];
    assert!(passage.contains("hotel in Lyon"));
    assert!(
        !passage.contains("quoted question"),
        "the quote is not theirs"
    );
    assert!(!passage.contains("Sam Jansen"), "nor the signature");
    drop(library);
    assert_eq!(
        app.resolve_writing_style("acct-1"),
        None,
        "which account drafts in it is this device's choice"
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::WritingStyle));
}
