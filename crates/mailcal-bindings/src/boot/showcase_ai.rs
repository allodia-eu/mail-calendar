//! What `MAILCAL_OFFER_AI` stages on an in-memory boot besides the offer, for the UI suites
//! (`docs/debugging.md`): two learned styles, an own endpoint on this device, sent mail on the
//! showcase's first account to learn from, and a transport that answers a draft from a canned
//! reply and every other request as no answer. Nothing leaves the device. Compiled with the
//! staging module, into a development build alone.

use std::collections::BTreeMap;

use engine_api::Provider;
use mailcal_account::StoredWritingStyle;
use mailcal_ai::{
    Exemplars, Habit, HttpRequest, HttpResponse, HttpTransport, LanguageCode, LanguageStyle,
    Provenance, StyleGuide, TransportFailed,
};
use mailcal_app::{
    App,
    ai::{AiEndpointChange, AiEndpointEdit, Class},
};

#[path = "showcase_ai_sent.rs"]
mod sent;

pub(crate) use sent::with_sent_mail;

/// The model the staged endpoint asks for. A draft asked of it gets the canned reply; asked of
/// any other model, or for anything but a draft, a request answers as no answer.
pub(crate) const CANNED_MODEL: &str = "showcase-canned";

/// The staged endpoint: a model on this device, declared as such.
const ADDRESS: &str = "http://localhost:11434/v1";

/// Stages the styles and the endpoint for `account`, whose address is `address`: a style whose
/// reveal was shown, which the account drafts in, and one whose reveal waits.
pub(crate) fn stage<P: Provider>(app: &App<P>, account: &str, address: &str) {
    app.add_writing_style(Some(account), shown(address));
    app.add_writing_style(Some(account), waiting(address));
    app.change_ai_endpoint_now(AiEndpointChange::Save(AiEndpointEdit {
        address: ADDRESS.to_owned(),
        model: CANNED_MODEL.to_owned(),
        declared: Some(Class::EuNative),
        key: String::new(),
    }));
}

/// The transport an offered in-memory boot sends through.
pub(crate) fn transport() -> Box<dyn HttpTransport> {
    Box::new(Canned)
}

struct Canned;

impl HttpTransport for Canned {
    fn post_json(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        let body: serde_json::Value =
            serde_json::from_str(request.body).map_err(|_| TransportFailed)?;
        // A draft is the one request whose answer carries a reply.
        let drafts = body["tools"][0]["function"]["parameters"]["properties"]
            .get("reply")
            .is_some();
        if body["model"] != CANNED_MODEL || !drafts {
            return Err(TransportFailed);
        }
        Ok(HttpResponse {
            status: 200,
            body: canned_draft(),
        })
    }
}

/// A draft with a summary and an item of each kind: a gap, a file to attach and a task.
fn canned_draft() -> String {
    let answered = serde_json::json!({
        "summary": "They ask whether the order can be delivered this month, and to whom.",
        "reply": "Thank you for the order.\n\nWe can deliver on [delivery date]. The signed \
                  quote is attached for your records.\n\nCould you tell me who will receive it?",
        "tasks": [
            { "kind": "attach", "text": "Attach the signed quote" },
            { "kind": "do", "text": "Book the delivery van" },
        ],
    });
    serde_json::json!({
        "choices": [{ "message": {
            "role": "assistant",
            "tool_calls": [{ "id": "call-1", "type": "function", "function": {
                "name": "emit_json",
                "arguments": answered.to_string(),
            }}],
        }}],
    })
    .to_string()
}

fn code(code: &str) -> LanguageCode {
    LanguageCode::new(code).expect("a two-letter code")
}

fn habits(habits: &[(&str, u8)]) -> Vec<Habit> {
    habits
        .iter()
        .map(|(text, share)| Habit {
            text: (*text).to_owned(),
            share: *share,
        })
        .collect()
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|entry| (*entry).to_owned()).collect()
}

/// English with every section: each of the reveal's pages has something for it.
fn english() -> LanguageStyle {
    LanguageStyle {
        greetings: habits(&[
            ("Hi [Name],", 55),
            ("Hello [Name],", 25),
            ("Dear [Name],", 10),
        ]),
        sign_offs: habits(&[("Best,", 60), ("Thanks,", 30)]),
        signs_as: "Sam".to_owned(),
        register: "Friendly and direct. Uses first names and contractions.".to_owned(),
        register_headline: "Friendly, direct".to_owned(),
        typical_words: 90,
        typical_paragraphs: 3,
        shape: "Short paragraphs, the answer first.".to_owned(),
        punctuation: "Few exclamation marks... Dashes rarely. Questions end a message.".to_owned(),
        structure: "Thanks the sender, answers, then says what happens next.".to_owned(),
        structure_headline: "Answer first".to_owned(),
        moves: "Declines with a reason and an alternative; chases after a week.".to_owned(),
        moves_headline: "Declines kindly".to_owned(),
        phrases: strings(&["Happy to help", "Let me know", "Speak soon"]),
        avoid: strings(&["Per my last email", "ASAP"]),
        ..LanguageStyle::default()
    }
}

/// Dutch with every section.
fn dutch() -> LanguageStyle {
    LanguageStyle {
        greetings: habits(&[("Hoi [Naam],", 50), ("Beste [Naam],", 40)]),
        sign_offs: habits(&[("Groet,", 70), ("Met vriendelijke groet,", 20)]),
        signs_as: "Sam".to_owned(),
        register: "Informal but polite. Uses je, never u, with colleagues.".to_owned(),
        register_headline: String::new(),
        typical_words: 60,
        typical_paragraphs: 2,
        shape: "Two short paragraphs.".to_owned(),
        punctuation: "Plain full stops? Rarely a question.".to_owned(),
        structure: "Comes straight to the point.".to_owned(),
        structure_headline: "Straight to the point".to_owned(),
        moves: "Asks again after three days.".to_owned(),
        moves_headline: "Chases politely".to_owned(),
        phrases: strings(&["Laat maar weten", "Fijne dag"]),
        avoid: strings(&["Zo spoedig mogelijk"]),
        ..LanguageStyle::default()
    }
}

/// German with greetings and sign-offs alone, so a per-language page says it has nothing for
/// German.
fn german() -> LanguageStyle {
    LanguageStyle {
        greetings: habits(&[("Hallo [Name],", 80)]),
        sign_offs: habits(&[("Viele Grüße", 90)]),
        ..LanguageStyle::default()
    }
}

fn style(
    name: &str,
    address: &str,
    languages: Vec<(&str, LanguageStyle, u32)>,
    revealed: bool,
) -> StoredWritingStyle {
    let mut guide = StyleGuide::new();
    let mut counts = BTreeMap::new();
    let mut exemplars = Exemplars::new();
    for (language, described, messages) in languages {
        guide.languages.insert(code(language), described);
        counts.insert(code(language), messages);
        exemplars.languages.insert(
            code(language),
            strings(&["Thanks for the update. I'll have a look this afternoon."]),
        );
    }
    guide.notes = "Keep replies under a hundred words.".to_owned();
    guide.learned = Some(Provenance {
        oldest: Some(1_755_000_000),
        newest: Some(1_763_000_000),
        messages_per_language: counts,
        learned_at: 1_763_500_000,
        ..Provenance::default()
    });
    StoredWritingStyle {
        name: name.to_owned(),
        learned_from: address.to_owned(),
        guide_json: serde_json::to_string(&guide).expect("a guide serialises"),
        exemplars_json: serde_json::to_string(&exemplars).expect("passages serialise"),
        revealed,
    }
}

/// The style whose reveal was shown: its row opens the detail.
fn shown(address: &str) -> StoredWritingStyle {
    style(
        "Work",
        address,
        vec![("en", english(), 48), ("nl", dutch(), 21)],
        true,
    )
}

/// The style whose reveal waits: its row opens the reveal, every page in English and Dutch.
fn waiting(address: &str) -> StoredWritingStyle {
    style(
        "Newly learned",
        address,
        vec![
            ("en", english(), 64),
            ("nl", dutch(), 30),
            ("de", german(), 9),
        ],
        false,
    )
}

#[cfg(test)]
#[path = "showcase_ai_learning_tests.rs"]
mod learning_tests;
#[cfg(test)]
#[path = "showcase_ai_offer_tests.rs"]
mod offer_tests;
#[cfg(test)]
#[path = "showcase_ai_reply_tests.rs"]
mod reply_tests;
#[cfg(test)]
#[path = "showcase_ai_tests.rs"]
mod tests;
