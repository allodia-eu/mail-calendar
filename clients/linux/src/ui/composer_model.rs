//! Pure request and quote-seed state for the Linux composer host.

use mailcal_bindings::{AgentDraft, MailtoPrefill, QuoteStyleKind, ReadingSnapshot};
use serde_json::{Value, json};

use super::{model::OpenedMessage, timestamps};
use crate::l10n;

/// Which shared rich-submit path a composer session uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComposeKind {
    New,
    Reply,
    ReplyAll,
    Forward,
}

/// Everything fixed when a single composer session opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposeRequest {
    pub(crate) kind: ComposeKind,
    pub(crate) account: Option<String>,
    pub(crate) key: Option<String>,
    pub(crate) initial_to: String,
    pub(crate) initial_cc: String,
    pub(crate) initial_bcc: String,
    pub(crate) subject: String,
    pub(crate) initial_body: Option<String>,
    pub(crate) quote: Option<String>,
    pub(crate) initial_from: Option<String>,
    /// Whether this client owns the body and should seed and offer its signature library.
    pub(crate) seeds_signature: bool,
    /// Files the composer opens already holding, from a share (`docs/os-integration.md`). Empty
    /// for every other route: the picker fills the list itself.
    pub(crate) files: Vec<PickedFile>,
}

impl ComposeRequest {
    pub(crate) fn from_mailto(prefill: MailtoPrefill, initial_from: Option<String>) -> Self {
        Self {
            kind: ComposeKind::New,
            account: None,
            key: None,
            initial_to: prefill.to,
            initial_cc: prefill.cc,
            initial_bcc: prefill.bcc,
            subject: prefill.subject,
            initial_body: (!prefill.body.is_empty()).then_some(prefill.body),
            quote: None,
            initial_from,
            seeds_signature: true,
            files: Vec::new(),
        }
    }

    pub(crate) fn from_agent(draft: AgentDraft, initial_from: Option<String>) -> Self {
        Self {
            kind: ComposeKind::New,
            account: None,
            key: None,
            initial_to: draft.to,
            initial_cc: draft.cc,
            initial_bcc: draft.bcc,
            subject: draft.subject,
            initial_body: (!draft.body_text.is_empty()).then_some(draft.body_text),
            quote: None,
            initial_from,
            seeds_signature: false,
            files: Vec::new(),
        }
    }

    /// Whether this composer opens with the caret in the message body rather than in To.
    ///
    /// A reply/forward is already addressed, so writing is the only thing left to do and the body
    /// takes it; a new message's To is empty and is where the user has to begin. A mail link is the
    /// exception among new messages; it supplied the recipient, so the body is the place there
    /// too. One predicate rather than two flags, because exactly one of the two may be focused
    /// (docs/contacts.md §4).
    pub(crate) fn opens_in_body(&self) -> bool {
        self.kind != ComposeKind::New || !self.initial_to.trim().is_empty()
    }
}

/// The shared editor call for a plain-text seed, encoded as JavaScript data rather than code.
pub(crate) fn plain_text_seed_script(body: Option<&str>) -> Option<String> {
    body.filter(|body| !body.is_empty())
        .map(|body| format!("window.setPlainText({});", json!(body)))
}

/// Metadata for one native file selected for an outgoing message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickedFile {
    pub(crate) path: String,
    pub(crate) file_name: String,
    pub(crate) media_type: String,
}

/// Editor and header values captured by the Send action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposerSubmission {
    pub(crate) request: ComposeRequest,
    pub(crate) to: String,
    pub(crate) cc: String,
    pub(crate) bcc: String,
    pub(crate) subject: String,
    pub(crate) document_json: String,
    pub(crate) files: Vec<PickedFile>,
    pub(crate) from: Option<String>,
}

/// Chooses the native From picker value using the same precedence as shared submission.
pub(crate) fn initial_sender(
    opened: Option<&OpenedMessage>,
    selected_account: Option<&str>,
    default_send_account: Option<String>,
) -> Option<String> {
    opened
        .map(|message| message.account.clone())
        .or_else(|| selected_account.map(str::to_owned))
        .or(default_send_account)
}

/// Builds the JSON payload accepted by the shared editor's `setComposerQuote` function.
///
/// `initial_text` pre-fills the lead paragraph above the quote. Only showcase mode supplies one, so
/// a real reply must carry no `initial_text` key at all: the editor treats an absent key and an
/// empty string differently only in where it parks the caret, but writing the key on every reply
/// would put a client-side default into a document the core owns.
pub(crate) fn quote_seed(
    message: &OpenedMessage,
    reading: &ReadingSnapshot,
    style: &QuoteStyleKind,
    is_forward: bool,
    initial_text: Option<&str>,
    zone: &str,
) -> Option<String> {
    if reading.key != message.key {
        return None;
    }
    let body_html = reading.html.as_deref().unwrap_or_default();
    let body_plain = reading.plain.as_deref().unwrap_or_default();
    if body_html.is_empty() && body_plain.is_empty() {
        return None;
    }

    // The reader of this quote is the *recipient*, so the date is localised exactly as the reading
    // header is (docs/timestamps.md). The core emits a UTC instant; sending it raw would put
    // `2026-08-31T05:01:00Z` in their mailbox.
    let sent = timestamps::local_date_time(&message.date, zone);
    let mut headers = vec![
        header(l10n::quote_from(), &message.from),
        header(l10n::quote_sent(), &sent),
    ];
    if !reading.to.is_empty() {
        headers.push(header(l10n::quote_to(), &reading.to));
    }
    if !reading.cc.is_empty() {
        headers.push(header(l10n::quote_cc(), &reading.cc));
    }
    headers.push(header(l10n::quote_subject(), &message.subject));

    let line = if is_forward {
        l10n::quote_forwarded().to_owned()
    } else {
        l10n::quote_attribution(&sent, &message.from)
    };
    let mut payload = json!({
        "style": match style {
            QuoteStyleKind::Indented => "Indented",
            QuoteStyleKind::LineAndHeader => "LineAndHeader",
        },
        "attribution": { "line": line, "headers": headers },
        "body_html": body_html,
        "body_plain": body_plain,
    });
    if let Some(text) = initial_text.filter(|text| !text.is_empty())
        && let Some(object) = payload.as_object_mut()
    {
        object.insert("initial_text".to_owned(), Value::String(text.to_owned()));
    }
    serde_json::to_string(&payload).ok()
}

fn header(label: &str, value: &str) -> Value {
    json!({ "label": label, "value": value })
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AgentDraft, MailtoPrefill, QuoteStyleKind, ReadingSnapshot};
    use serde_json::Value;

    use super::{ComposeKind, ComposeRequest, initial_sender, plain_text_seed_script, quote_seed};
    use crate::ui::model::OpenedMessage;

    #[test]
    fn a_mail_link_becomes_an_editable_new_message_without_interpreting_its_body_as_code() {
        let request = ComposeRequest::from_mailto(
            MailtoPrefill {
                to: "ada@example.test".to_owned(),
                cc: "copy@example.test".to_owned(),
                bcc: "audit@example.test".to_owned(),
                subject: "Lunch".to_owned(),
                body: "Hello </script>\nSecond line".to_owned(),
            },
            Some("account".to_owned()),
        );
        assert_eq!(request.kind, ComposeKind::New);
        assert_eq!(request.initial_bcc, "audit@example.test");
        assert_eq!(
            request.initial_body.as_deref(),
            Some("Hello </script>\nSecond line")
        );
        assert_eq!(request.initial_from.as_deref(), Some("account"));
        assert_eq!(
            plain_text_seed_script(request.initial_body.as_deref()).as_deref(),
            Some("window.setPlainText(\"Hello </script>\\nSecond line\");")
        );
        assert_eq!(plain_text_seed_script(None), None);
        assert_eq!(plain_text_seed_script(Some("")), None);
        assert!(request.seeds_signature);
    }

    #[test]
    fn an_assistant_draft_is_unsent_new_mail_without_a_second_signature() {
        let request = ComposeRequest::from_agent(
            AgentDraft {
                account: Some("work".to_owned()),
                to: "ada@example.test".to_owned(),
                cc: "copy@example.test".to_owned(),
                bcc: "audit@example.test".to_owned(),
                subject: "Planning".to_owned(),
                body_text: "A complete body and sign-off".to_owned(),
                reply_to_account: Some("work".to_owned()),
                reply_to_key: Some("original".to_owned()),
            },
            Some("work".to_owned()),
        );

        assert_eq!(request.kind, ComposeKind::New);
        assert_eq!(request.account, None);
        assert_eq!(request.key, None);
        assert_eq!(request.initial_from.as_deref(), Some("work"));
        assert_eq!(request.initial_bcc, "audit@example.test");
        assert_eq!(
            request.initial_body.as_deref(),
            Some("A complete body and sign-off")
        );
        assert!(!request.seeds_signature);
    }

    #[test]
    fn quote_seed_preserves_sanitized_body_and_structures_headers_as_data() {
        assert_ne!(ComposeKind::Reply, ComposeKind::Forward);
        assert_ne!(ComposeKind::New, ComposeKind::ReplyAll);
        let message = OpenedMessage {
            account: "account".to_owned(),
            key: "message".to_owned(),
            subject: "Planning <script>".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };
        let reading = ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: "message".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            to: "recipient@example.test".to_owned(),
            cc: "copy@example.test".to_owned(),
            bcc: String::new(),
            html: Some("<p>Already sanitized</p>".to_owned()),
            plain: Some("Already sanitized".to_owned()),
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        };

        let json = quote_seed(
            &message,
            &reading,
            &QuoteStyleKind::Indented,
            false,
            None,
            "Europe/Amsterdam",
        )
        .expect("matching body produces a quote");
        let payload: Value = serde_json::from_str(&json).expect("valid JSON");

        assert_eq!(payload["style"], "Indented");
        assert_eq!(payload["body_html"], "<p>Already sanitized</p>");
        assert_eq!(payload["body_plain"], "Already sanitized");
        assert_eq!(
            payload["attribution"]["headers"][2]["value"],
            "recipient@example.test"
        );
        assert!(payload.get("initial_text").is_none());
    }

    #[test]
    fn the_quoted_date_is_localised_before_it_is_sent_to_anyone() {
        // The attribution and the `Sent:` header are read by the *recipient*, so a raw UTC instant
        // from the core is a defect in their mailbox rather than in ours.
        let message = OpenedMessage {
            account: "account".to_owned(),
            key: "message".to_owned(),
            subject: "Planning".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            date: "2026-08-31T05:01:00Z".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };
        let reading = ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: "message".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            to: "recipient@example.test".to_owned(),
            cc: String::new(),
            bcc: String::new(),
            html: Some("<p>Body</p>".to_owned()),
            plain: Some("Body".to_owned()),
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        };

        let json = quote_seed(
            &message,
            &reading,
            &QuoteStyleKind::Indented,
            false,
            None,
            "Europe/Amsterdam",
        )
        .expect("matching body produces a quote");
        let payload: Value = serde_json::from_str(&json).expect("valid JSON");

        let line = payload["attribution"]["line"]
            .as_str()
            .expect("an attribution line");
        let sent = payload["attribution"]["headers"][1]["value"]
            .as_str()
            .expect("a Sent header");
        for value in [line, sent] {
            assert!(
                !value.contains('T'),
                "raw ISO instant reached the quote: {value}"
            );
            assert!(
                !value.contains('Z'),
                "raw ISO instant reached the quote: {value}"
            );
        }
        // Amsterdam is UTC+2 on 31 August, so the localised hour is the visible proof that the
        // instant was converted rather than merely reformatted.
        assert!(
            line.contains("07:01"),
            "not converted to the display zone: {line}"
        );
        assert!(
            sent.contains("07:01"),
            "not converted to the display zone: {sent}"
        );
    }

    #[test]
    fn only_a_supplied_initial_text_reaches_the_seed() {
        let message = OpenedMessage {
            account: "account".to_owned(),
            key: "message".to_owned(),
            subject: "Subject".to_owned(),
            from: "sender@example.test".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };
        let reading = ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: "message".to_owned(),
            from: String::new(),
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            html: Some("<p>Body</p>".to_owned()),
            plain: None,
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        };
        let seed = |text| {
            let json = quote_seed(
                &message,
                &reading,
                &QuoteStyleKind::Indented,
                false,
                text,
                "Europe/Amsterdam",
            )
            .expect("matching body produces a quote");
            serde_json::from_str::<Value>(&json).expect("valid JSON")
        };

        assert_eq!(seed(Some("Sounds good"))["initial_text"], "Sounds good");
        // An empty string is not a pre-filled body; it must leave the key off entirely, as on
        // every other client.
        assert!(seed(Some("")).get("initial_text").is_none());
        assert!(seed(None).get("initial_text").is_none());
    }

    #[test]
    fn quote_seed_rejects_a_stale_or_empty_reading_snapshot() {
        let message = OpenedMessage {
            account: "account".to_owned(),
            key: "current".to_owned(),
            subject: String::new(),
            from: String::new(),
            date: String::new(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };
        let mut reading = ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: "stale".to_owned(),
            from: String::new(),
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            html: Some("<p>Body</p>".to_owned()),
            plain: None,
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        };

        assert!(
            quote_seed(
                &message,
                &reading,
                &QuoteStyleKind::Indented,
                false,
                None,
                "Europe/Amsterdam"
            )
            .is_none()
        );
        reading.key = "current".to_owned();
        reading.html = None;
        assert!(
            quote_seed(
                &message,
                &reading,
                &QuoteStyleKind::Indented,
                false,
                None,
                "Europe/Amsterdam"
            )
            .is_none()
        );
    }

    #[test]
    fn sender_prefers_message_then_selected_mailbox_then_app_default() {
        let message = OpenedMessage {
            account: "message-account".to_owned(),
            key: "message".to_owned(),
            subject: String::new(),
            from: String::new(),
            date: String::new(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };

        assert_eq!(
            initial_sender(
                Some(&message),
                Some("selected-account"),
                Some("default-account".to_owned()),
            ),
            Some("message-account".to_owned())
        );
        assert_eq!(
            initial_sender(
                None,
                Some("selected-account"),
                Some("default-account".to_owned()),
            ),
            Some("selected-account".to_owned())
        );
        assert_eq!(
            initial_sender(None, None, Some("default-account".to_owned())),
            Some("default-account".to_owned())
        );
    }
}
