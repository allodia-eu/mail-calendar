//! What the Draft a reply tests drive the core with: a host on the test's own runtime, a relay
//! that answers from a script and reaches no network, an app with a style, and a reply composer.
//!
//! A child of the rich-submit tests, for [`super::mail_fake`].

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, mpsc},
};

use mailcal_ai::{
    AiBackend, AiError, HttpRequest, HttpResponse, HttpTransport, StyleGuide, TransportFailed,
    wire::{ChatRequest, ChatResponse},
};
use mailcal_composer::{LeadText, ReplyDraftReport};
use serde_json::json;

use super::mail_fake::{ProviderLogs, ThreadProvider, app_and_logs, staging_dir};
use crate::{
    App, ComposeOrigin, CompositionId, Intent, MessageRef,
    ai::{
        AiCancel, AiHost, AiQueued, AiRelay, BuildKind, KeyNotStored, RelayStatus, ReplyDraftStart,
        ReplyDraftView, StoredWritingStyle,
    },
};

/// A host whose runtime is the test's own, so the engine's reads run where its store lives.
pub(super) struct RuntimeHost(pub(super) tokio::runtime::Handle);

/// A socket that reaches nothing: the scripted relay never uses it.
struct Unplugged;

impl HttpTransport for Unplugged {
    fn post_json(&self, _request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        Err(TransportFailed)
    }
}

impl AiHost for RuntimeHost {
    fn transport(&self, _cancel: AiCancel) -> Box<dyn HttpTransport> {
        Box::new(Unplugged)
    }

    fn wait(&self, work: Pin<Box<dyn Future<Output = ()> + Send + '_>>) {
        self.0.block_on(work);
    }

    fn store_endpoint_key(&self, _key: Option<&str>) -> Result<(), KeyNotStored> {
        Ok(())
    }
}

/// A relay answering every request from `answer`, recording each, and holding each answer until
/// `release` is sent when it was built held.
pub(super) struct ScriptedRelay {
    answer: Mutex<Result<ChatResponse, AiError>>,
    pub(super) seen: Arc<Mutex<Vec<ChatRequest>>>,
    hold: Mutex<Option<mpsc::Receiver<()>>>,
}

impl ScriptedRelay {
    pub(super) fn answering(answer: Result<ChatResponse, AiError>) -> Arc<Self> {
        Arc::new(Self {
            answer: Mutex::new(answer),
            seen: Arc::default(),
            hold: Mutex::new(None),
        })
    }

    pub(super) fn held(answer: Result<ChatResponse, AiError>) -> (Arc<Self>, mpsc::Sender<()>) {
        let (release, wait) = mpsc::channel();
        let relay = Self::answering(answer);
        *relay.hold.lock().unwrap() = Some(wait);
        (relay, release)
    }
}

struct ScriptedBackend(Arc<ScriptedRelay>);

impl AiBackend for ScriptedBackend {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError> {
        self.0.seen.lock().unwrap().push(request.clone());
        if let Some(wait) = self.0.hold.lock().unwrap().take() {
            wait.recv().unwrap();
        }
        self.0.answer.lock().unwrap().clone()
    }
}

impl AiRelay for Arc<ScriptedRelay> {
    fn status(&self) -> RelayStatus {
        RelayStatus::Ready
    }

    fn backend(&self, _transport: Box<dyn HttpTransport>) -> Box<dyn AiBackend> {
        Box::new(ScriptedBackend(Arc::clone(self)))
    }
}

/// The model's answer: a reply with two gaps, an attach item and a thing to do.
pub(super) fn drafted() -> ChatResponse {
    let arguments = json!({
        "summary": "Anna asks whether Friday still works.",
        "reply": "Hi Anna,\n\nFriday at [time] works; the fee is [amount].",
        "tasks": [
            { "kind": "attach", "text": "Attach the agenda" },
            { "kind": "do", "text": "Book the room" },
        ],
    });
    serde_json::from_value(json!({
        "choices": [{ "message": {
            "role": "assistant",
            "tool_calls": [{ "id": "call-1", "type": "function", "function": {
                "name": "emit_json",
                "arguments": arguments.to_string(),
            }}],
        }}],
    }))
    .unwrap()
}

/// An app over `provider` in a development build, with a style for its account and the relay.
pub(super) async fn assistant(
    provider: ThreadProvider,
    relay: &Arc<ScriptedRelay>,
) -> (Arc<App<ThreadProvider>>, ProviderLogs) {
    let (app, logs) = app_and_logs(provider);
    app.dispatch(Intent::RefreshMail).await;
    app.install_ai_host(
        Arc::new(RuntimeHost(tokio::runtime::Handle::current())),
        BuildKind::Development,
    );
    app.install_ai_relay(Arc::new(Arc::clone(relay)));
    learn_a_style(&app);
    (app, logs)
}

pub(super) fn learn_a_style(app: &App<ThreadProvider>) {
    app.add_writing_style(
        Some("acct-1"),
        StoredWritingStyle {
            name: "Mine".to_owned(),
            learned_from: "me@allodia.local".to_owned(),
            guide_json: serde_json::to_string(&StyleGuide::new()).unwrap(),
            exemplars_json: String::new(),
            revealed: true,
        },
    );
}

/// A reply composer opened on `m1`.
pub(super) async fn open_reply(app: &App<ThreadProvider>, name: &str) -> CompositionId {
    let scratch = tempfile::tempdir().unwrap();
    let composition = CompositionId::new(name).unwrap();
    app.open_composer(
        composition.clone(),
        ComposeOrigin::Reply {
            message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
            all: false,
        },
        None,
        &staging_dir(&scratch, name),
    )
    .await
    .expect("the reply opens");
    composition
}

pub(super) fn view(app: &App<ThreadProvider>, composition: &CompositionId) -> ReplyDraftView {
    app.composer_reply_draft(composition, None, &|_| 0)
        .expect("offered on a reply")
}

pub(super) fn report(
    serial: u64,
    lead: LeadText,
    inserted: bool,
    left: Vec<u32>,
    edits: u64,
) -> ReplyDraftReport {
    ReplyDraftReport {
        serial,
        lead,
        inserted,
        left,
        edits,
    }
}

pub(super) fn start(replace: bool) -> ReplyDraftStart {
    ReplyDraftStart {
        from: None,
        instruction: "  yes, Friday  ".to_owned(),
        language: "en".to_owned(),
        replace,
    }
}

/// Runs the work a press queued, off the test's runtime, as the bindings do.
pub(super) async fn run(app: &Arc<App<ThreadProvider>>) {
    let app = Arc::clone(app);
    tokio::task::spawn_blocking(move || app.run_ai_work())
        .await
        .unwrap();
}

/// Drafts into a reply whose lead is empty, and has the editor put it in.
pub(super) async fn drafted_into(
    app: &Arc<App<ThreadProvider>>,
    composition: &CompositionId,
) -> u64 {
    app.take_reply_draft_report(composition, report(0, LeadText::Empty, false, vec![], 0), 0);
    assert_eq!(
        app.draft_reply(composition, start(false)),
        Some(AiQueued::Run)
    );
    run(app).await;
    let serial = view(app, composition)
        .editor_script
        .expect("a draft for the editor")
        .serial;
    app.take_reply_draft_report(
        composition,
        report(serial, LeadText::Draft, true, vec![0, 1], 1),
        0,
    );
    serial
}
