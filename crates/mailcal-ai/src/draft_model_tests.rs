//! A draft through models that do not behave as the instructions ask: working notes in place of
//! the tool call, a forced tool refused, a reasoning model's thinking, a second closing above the
//! signature, and a thread padded out by a plain-text conversion.

use std::sync::Arc;

use mailcal_jurisdiction::{Class, Mode};

use super::{DraftRequest, ThreadMessage, draft_reply};
use crate::{
    AiError, Exemplars, GatedBackend, Habit, LanguageStyle, OwnEndpoint, StyleGuide,
    test_support::{CannedTransport, gated, text_answer},
    wire::{ChatRequest, ChatResponse},
};

const SIGNATURE: &str = "Met vriendelijke groet,\nSanne de Vries\nAllodia";

fn guide() -> StyleGuide {
    let mut guide = StyleGuide::new();
    guide.languages.insert(
        "nl".to_owned(),
        LanguageStyle {
            sign_offs: vec![
                Habit {
                    text: "Groeten,".to_owned(),
                    share: 60,
                },
                Habit {
                    text: "Met vriendelijke groet".to_owned(),
                    share: 30,
                },
            ],
            signs_as: "Sanne".to_owned(),
            typical_words: 60,
            ..LanguageStyle::default()
        },
    );
    guide
}

fn thread(body: &str) -> Vec<ThreadMessage> {
    vec![ThreadMessage {
        from: "Marc <marc@example.nl>".to_owned(),
        date: "2 Jul".to_owned(),
        body: body.to_owned(),
    }]
}

fn request<'a>(
    thread: &'a [ThreadMessage],
    guide: &'a StyleGuide,
    exemplars: &'a Exemplars,
    recipient_messages: &'a [String],
    signature: Option<&'a str>,
) -> DraftRequest<'a> {
    DraftRequest {
        thread,
        guide,
        exemplars,
        recipient_messages,
        intent: None,
        language: Some("nl"),
        signature,
        ui_language: "en",
    }
}

fn drafted(answer: &str, signature: Option<&str>) -> (Result<String, AiError>, ChatRequest) {
    let (backend, seen) = gated(vec![Ok(text_answer(answer))]);
    let (thread, guide, exemplars) = (
        thread("Kun je de tekeningen sturen?"),
        guide(),
        Exemplars::new(),
    );
    let draft = draft_reply(
        &request(&thread, &guide, &exemplars, &[], signature),
        &backend,
    );
    let sent = seen.lock().unwrap()[0].clone();
    (draft.map(|draft| draft.text), sent)
}

#[test]
fn working_notes_in_place_of_the_tool_call_are_not_a_draft() {
    let notes = "We need to draft reply in Dutch. The person signs as Sanne. Use emit_json with \
                 summary, reply and tasks. Now produce JSON.";
    assert_eq!(drafted(notes, None).0, Err(AiError::Malformed));
}

#[test]
fn working_notes_that_quote_the_instructions_are_not_a_draft() {
    // A reasoning model thinking aloud about its instructions, without naming the tool.
    let notes = "The user wants me to draft an email reply. Re-reading the setup: \"You draft \
                 email replies in the voice of one person, described below, so that they only \
                 need to check\". But the email is addressed to someone else. Wait.";
    assert_eq!(drafted(notes, None).0, Err(AiError::Malformed));
}

#[test]
fn a_reply_that_repeats_a_long_signature_line_is_still_a_draft() {
    let signature = "Sanne de Vries\nThis message is meant only for the person it is addressed to.";
    let reply = "Hoi Marc,\n\nIk stuur ze je morgen.\n\nThis message is meant only for the person \
                 it is addressed to.";
    let (text, _) = drafted(reply, Some(signature));
    assert!(text.unwrap().starts_with("Hoi Marc,"));
}

#[test]
fn a_plain_answer_cut_off_at_the_length_limit_is_not_a_draft() {
    let cut_off: ChatResponse = serde_json::from_value(serde_json::json!({
        "choices": [{
            "message": { "role": "assistant", "content": "Let me analyse this. Marc asks" },
            "finish_reason": "length",
        }],
    }))
    .unwrap();
    let (backend, _) = gated(vec![Ok(cut_off)]);
    let (thread, guide, exemplars) = (
        thread("Kun je de tekeningen sturen?"),
        guide(),
        Exemplars::new(),
    );
    let draft = draft_reply(&request(&thread, &guide, &exemplars, &[], None), &backend);
    assert_eq!(draft.map(|draft| draft.text), Err(AiError::Malformed));
}

#[test]
fn a_plain_reply_from_a_server_that_ignores_tools_is_still_a_draft() {
    let (text, _) = drafted("Hoi Marc,\n\nIk stuur ze je [wanneer].", None);
    assert_eq!(text.unwrap(), "Hoi Marc,\n\nIk stuur ze je [wanneer].");
}

#[test]
fn the_ceiling_leaves_a_reasoning_model_room_to_think_before_it_answers() {
    let (_, sent) = drafted("Hoi Marc,\n\nPrima.", None);
    let ceiling = sent.max_tokens.unwrap();
    assert!((3_600..=6_600).contains(&ceiling), "{ceiling}");
}

#[test]
fn a_one_line_sign_off_above_the_signature_is_taken_off() {
    let (text, _) = drafted(
        "Hoi Marc,\n\nIk stuur ze je [wanneer].\n\nGroeten",
        Some(SIGNATURE),
    );
    assert_eq!(text.unwrap(), "Hoi Marc,\n\nIk stuur ze je [wanneer].");
}

#[test]
fn without_a_signature_the_sign_off_is_the_closing_and_stays() {
    let answer = "Hoi Marc,\n\nIk stuur ze je [wanneer].\n\nGroeten";
    assert_eq!(drafted(answer, None).0.unwrap(), answer);
}

#[test]
fn the_thread_and_the_recipient_context_go_in_without_padding() {
    let (backend, seen) = gated(vec![Ok(text_answer("Hoi Marc,\n\nPrima."))]);
    let thread = thread(
        "Kun je de tekeningen sturen?\r\n\r\n\r\n\r\nMarc Jansen  \r\nexample.nl<https://example.nl>\r\n",
    );
    let recipient = ["Hoi Marc,\r\n\r\n\r\nPrima.  \r\n".to_owned()];
    let (guide, exemplars) = (guide(), Exemplars::new());
    draft_reply(
        &request(&thread, &guide, &exemplars, &recipient, None),
        &backend,
    )
    .unwrap();

    let material = &seen.lock().unwrap()[0].messages[1].content;
    assert!(material.contains("sturen?\n\nMarc Jansen\nexample.nl\n"));
    assert!(material.contains("Hoi Marc,\n\nPrima."));
    assert!(!material.contains('\r'));
    assert!(!material.contains("<https://"));
    assert!(!material.contains("\n\n\n"));
}

#[test]
fn a_model_that_refuses_the_forced_tool_is_asked_again_and_its_json_is_read() {
    let answered = serde_json::json!({ "choices": [{ "message": { "content":
        "{\"summary\": \"Marc asks for the drawings.\", \"reply\": \"Hoi Marc,\\n\\nIk stuur ze.\", \"tasks\": []}"
    } }] });
    let transport = CannedTransport::after(
        &[(
            400,
            r#"{"error":{"message":"Model 'deepseek-v4.1-flash' does not support tool_choice."}}"#,
        )],
        200,
        &answered.to_string(),
    );
    let sent = Arc::clone(&transport.sent);
    let backend = GatedBackend::own_endpoint(
        OwnEndpoint::new(
            "https://api.example.eu/v1",
            None,
            "deepseek-v4.1-flash",
            Some(Class::EuNative),
        )
        .unwrap(),
        Box::new(transport),
        Arc::new(|| Mode::EuNative),
    );
    let (thread, guide, exemplars) = (
        thread("Kun je de tekeningen sturen?"),
        guide(),
        Exemplars::new(),
    );

    let draft = draft_reply(&request(&thread, &guide, &exemplars, &[], None), &backend).unwrap();

    assert_eq!(draft.text, "Hoi Marc,\n\nIk stuur ze.");
    assert_eq!(draft.summary, "Marc asks for the drawings.");
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].body.get("tool_choice").is_none());
}
