//! An app to learn from for these tests: accounts whose Sent folder holds mail long enough to
//! learn from, an own endpoint set up, and a host whose model answers with a description. Nothing
//! reaches the network.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
};

use engine_api::AccountId;
use mailcal_ai::{Class, HttpRequest, HttpResponse, HttpTransport, TransportFailed};

use super::{LearningIntent, LearningStep, LearningView};
use crate::{
    App, Surface,
    ai::{AiCancel, AiEndpointChange, AiEndpointEdit, AiHost, BuildKind, KeyNotStored},
    tests_fakes::{FakeProvider, account, app, message},
};

/// A sent message's source: a reply with well over forty words of its author's own.
pub(super) const REPLY: &str = "From: me@acct-1.local\r\nTo: anna@example.eu\r\n\
Subject: Re: Planning\r\nMessage-ID: <reply@acct-1.local>\r\nContent-Type: text/plain; \
charset=utf-8\r\n\r\nHi Anna,\r\n\r\nThank you for sending the planning over so quickly. I have \
read it twice and it looks sound to me. The only point I would change is the date of the second \
review, which falls in the week most of the team is away. Could we move it to the Tuesday after, \
or would that put the delivery at risk? Let me know what suits you and I will update the \
calendar.\r\n\r\nBest regards,\r\nSam\r\n";

/// The description a model answers with.
const DESCRIBED: &str = r#"{"style":{"greetings":[{"text":"Hi [Name],","share":70}],
"sign_offs":[{"text":"Best regards,","share":60}],"signs_as":"Sam",
"register":"Informal and warm. Uses first names.","typical_words":80,"typical_paragraphs":2,
"phrases":["Let me know"],"avoid":[]},"exemplars":[]}"#;

/// What the model is told to do with each request.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// Describes the person.
    Describe,
    /// Answers `503`.
    Unavailable,
}

/// Where each request says it arrived, and what it waits on to go on.
type Gate = (Sender<()>, Arc<Mutex<Receiver<()>>>);

/// A host whose transport answers as [`Answer`] says, optionally waiting at each request until a
/// test lets it go.
pub(super) struct LearningHost {
    pub(super) answer: Mutex<Answer>,
    pub(super) asked: Mutex<u32>,
    gate: Mutex<Option<Gate>>,
}

impl LearningHost {
    fn new() -> Self {
        Self {
            answer: Mutex::new(Answer::Describe),
            asked: Mutex::new(0),
            gate: Mutex::new(None),
        }
    }

    /// Holds each request until `release` sends; `entered` hears of each as it arrives.
    pub(super) fn hold(&self) -> (Receiver<()>, Sender<()>) {
        let (entered, heard) = channel();
        let (release, held) = channel();
        *self.gate.lock().unwrap() = Some((entered, Arc::new(Mutex::new(held))));
        (heard, release)
    }
}

struct Transport {
    host: Arc<LearningHost>,
    cancel: AiCancel,
}

impl HttpTransport for Transport {
    fn post_json(&self, _request: HttpRequest<'_>) -> Result<HttpResponse, TransportFailed> {
        *self.host.asked.lock().unwrap() += 1;
        let gate = self.host.gate.lock().unwrap().clone();
        if let Some((entered, held)) = gate {
            entered.send(()).unwrap();
            held.lock().unwrap().recv().unwrap();
        }
        if self.cancel.is_set() {
            return Err(TransportFailed);
        }
        let answer = *self.host.answer.lock().unwrap();
        Ok(match answer {
            Answer::Describe => HttpResponse {
                status: 200,
                body: serde_json::json!({"choices": [{"message": {"role": "assistant",
                    "tool_calls": [{"id": "c1", "type": "function", "function": {
                        "name": "emit_json", "arguments": DESCRIBED}}]}}]})
                .to_string(),
            },
            Answer::Unavailable => HttpResponse {
                status: 503,
                body: String::new(),
            },
        })
    }
}

/// The host's half, shared with the transports it hands out.
struct Host(Arc<LearningHost>);

impl AiHost for Host {
    fn transport(&self, cancel: AiCancel) -> Box<dyn HttpTransport> {
        Box::new(Transport {
            host: Arc::clone(&self.0),
            cancel,
        })
    }

    fn wait(&self, work: Pin<Box<dyn Future<Output = ()> + Send + '_>>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(work);
    }

    fn store_endpoint_key(&self, _key: Option<&str>) -> Result<(), KeyNotStored> {
        Ok(())
    }
}

/// An app over `accounts` sent-mail accounts, the first's Sent folder holding `sent`, with an own
/// endpoint set up and every account synced.
pub(super) struct Learner {
    pub(super) app: Arc<App<FakeProvider>>,
    pub(super) host: Arc<LearningHost>,
    pub(super) surfaces: Arc<Mutex<Vec<Surface>>>,
}

impl Learner {
    pub(super) async fn new(ids: &[&str], sent: Vec<engine_api::Message>) -> Self {
        let providers = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let mail = if index == 0 {
                    sent.clone()
                } else {
                    vec![message(&format!("{id}-m"), "sent", "Sent elsewhere")]
                };
                (
                    *id,
                    FakeProvider::with_sent_and_archive(mail).with_source(REPLY.as_bytes()),
                )
            })
            .collect();
        Self::over(providers).await
    }

    /// An app over these accounts, each served by its provider.
    pub(super) async fn over(providers: Vec<(&str, FakeProvider)>) -> Self {
        let surfaces = Arc::new(Mutex::new(Vec::new()));
        let ids: Vec<&str> = providers.iter().map(|(id, _)| *id).collect();
        let accounts = providers
            .into_iter()
            .map(|(id, provider)| account(id, provider))
            .collect();
        let app = Arc::new(app(accounts, &surfaces));
        for id in ids {
            app.sync_account(&AccountId::try_from(id).unwrap()).await;
        }
        let host = Arc::new(LearningHost::new());
        app.install_ai_host(Arc::new(Host(Arc::clone(&host))), BuildKind::Development);
        app.change_ai_endpoint_now(AiEndpointChange::Save(AiEndpointEdit {
            address: "https://models.example.eu/v1".to_owned(),
            model: "m".to_owned(),
            declared: Some(Class::EuNative),
            key: None,
        }));
        surfaces.lock().unwrap().clear();
        Self {
            app,
            host,
            surfaces,
        }
    }

    /// Sends `intent`, starting a read on this thread's runtime and leaving a run to the caller.
    pub(super) async fn intent(&self, intent: LearningIntent) -> LearningStep<FakeProvider> {
        match self.app.writing_style_learning_intent(intent) {
            LearningStep::Read(ticket) => {
                self.app.read_learning_review(ticket).await;
                LearningStep::Nothing
            }
            other => other,
        }
    }

    /// Sends `intent` and runs any run it queues to its end, on this thread.
    pub(super) async fn press(&self, intent: LearningIntent) {
        match self.intent(intent).await {
            LearningStep::Learn(work) => self.app.run_ai_work_now(work),
            LearningStep::Change(change) => self.app.change_writing_style_now(change),
            LearningStep::Nothing | LearningStep::Read(_) => {}
        }
    }

    pub(super) fn view(&self) -> LearningView {
        self.app.writing_style_learning().expect("a flow on screen")
    }
}

/// The consent, in English.
pub(super) fn learn() -> LearningIntent {
    LearningIntent::Learn {
        name: "My style".to_owned(),
        language: "en".to_owned(),
    }
}

/// A message in the Sent folder, sent yesterday.
pub(super) fn sent(id: &str) -> engine_api::Message {
    let yesterday = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    let mut sent = message(id, "sent", "Re: Planning");
    sent.sent_at = Some(
        engine_api::UtcDateTime::new(
            yesterday.year(),
            u8::from(yesterday.month()),
            yesterday.day(),
            9,
            0,
            0,
        )
        .unwrap(),
    );
    sent
}
