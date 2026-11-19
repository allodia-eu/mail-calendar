//! The assistant's queue: one piece of work at a time, the same work asked twice started once,
//! stopping what waits and what runs, and the engine read through the host's runtime.

use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, channel},
};

use mailcal_ai::{
    Class,
    wire::{ChatMessage, ChatRequest, Purpose},
};

use super::{
    AiEndpointChange, AiEndpointEdit, AiQueued, AiWork, AiWorkKey, BuildKind,
    tests_support::Fixture,
};
use crate::{App, CompositionId, Surface, tests_fakes::FakeProvider};

fn drafting(id: &str) -> AiWorkKey {
    AiWorkKey::Drafting(CompositionId::new(id).unwrap())
}

/// Work that records it ran under `label`.
fn noting(
    key: AiWorkKey,
    label: &'static str,
    ran: &Arc<Mutex<Vec<&'static str>>>,
) -> AiWork<FakeProvider> {
    let ran = Arc::clone(ran);
    AiWork::new(key, move |_, _| ran.lock().unwrap().push(label))
}

#[test]
fn work_runs_one_at_a_time_and_the_same_work_asked_twice_runs_once() {
    let fixture = Fixture::new(BuildKind::Development);
    let ran = Arc::new(Mutex::new(Vec::new()));
    let app = &fixture.app;

    assert_eq!(
        app.queue_ai_work(noting(AiWorkKey::Learning, "learn", &ran)),
        AiQueued::Run
    );
    assert!(fixture.signalled(Surface::WritingStyle));
    assert_eq!(
        app.queue_ai_work(noting(AiWorkKey::Learning, "learn again", &ran)),
        AiQueued::Duplicate
    );
    assert_eq!(
        app.queue_ai_work(noting(drafting("c1"), "draft", &ran)),
        AiQueued::Waiting
    );
    assert!(fixture.signalled(Surface::ReplyDraft));
    assert_eq!(app.ai_work(), [AiWorkKey::Learning, drafting("c1")]);
    assert!(app.ai_work_running(&AiWorkKey::Learning));
    assert!(!app.ai_work_running(&drafting("c1")));

    app.run_ai_work();
    assert_eq!(*ran.lock().unwrap(), ["learn", "draft"]);
    assert_eq!(app.ai_work(), []);
}

#[test]
fn stopping_work_that_waits_takes_it_out_of_the_queue() {
    let fixture = Fixture::new(BuildKind::Development);
    let ran = Arc::new(Mutex::new(Vec::new()));
    let app = &fixture.app;
    app.queue_ai_work(noting(AiWorkKey::Learning, "learn", &ran));
    app.queue_ai_work(noting(drafting("c1"), "draft", &ran));
    fixture.forget_surfaces();

    assert!(app.cancel_ai_work(&drafting("c1")));
    assert!(fixture.signalled(Surface::ReplyDraft));
    assert!(!app.cancel_ai_work(&drafting("c2")));
    app.run_ai_work();
    assert_eq!(*ran.lock().unwrap(), ["learn"]);
}

/// Work that runs, told to stop: its request in flight is abandoned and the work reads that it
/// was stopped. Asked for again while it winds down, it is queued afresh rather than dropped.
#[test]
fn stopping_work_that_runs_abandons_its_request() {
    let fixture = Arc::new(Fixture::new(BuildKind::Development));
    fixture
        .app
        .change_ai_endpoint_now(AiEndpointChange::Save(AiEndpointEdit {
            address: "https://models.example.eu/v1".to_owned(),
            model: "m".to_owned(),
            declared: Some(Class::EuNative),
            key: None,
        }));
    let (started, wait_for_start) = channel::<()>();
    let (stop, wait_for_stop): (_, Receiver<()>) = channel();
    let stopped_seen = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&stopped_seen);
    let work = AiWork::new(drafting("c1"), move |app: &App<FakeProvider>, run| {
        started.send(()).unwrap();
        wait_for_stop.recv().unwrap();
        let backend = app.ai_backend(run).unwrap();
        let answer = backend.chat(&ChatRequest {
            purpose: Purpose::Draft,
            messages: vec![ChatMessage::user("Hello")],
            tools: Vec::new(),
            tool_choice: None,
            temperature: None,
            max_tokens: None,
        });
        *seen.lock().unwrap() = Some((run.cancelled(), answer.is_err()));
    });
    assert_eq!(fixture.app.queue_ai_work(work), AiQueued::Run);
    let runner = {
        let fixture = Arc::clone(&fixture);
        std::thread::spawn(move || fixture.app.run_ai_work())
    };
    wait_for_start.recv().unwrap();

    assert!(fixture.app.cancel_ai_work(&drafting("c1")));
    assert!(!fixture.app.ai_work_running(&drafting("c1")));
    assert_eq!(fixture.app.ai_work(), []);
    let ran = Arc::new(Mutex::new(Vec::new()));
    assert_eq!(
        fixture
            .app
            .queue_ai_work(noting(drafting("c1"), "again", &ran)),
        AiQueued::Waiting
    );
    stop.send(()).unwrap();
    runner.join().unwrap();

    assert_eq!(*stopped_seen.lock().unwrap(), Some((true, true)));
    assert!(fixture.host.seen.lock().unwrap().stopped);
    assert_eq!(*ran.lock().unwrap(), ["again"]);
}

/// The engine's reads run on the host's runtime, between requests.
#[test]
fn work_reads_the_engine_through_the_hosts_runtime() {
    let fixture = Fixture::new(BuildKind::Development);
    let read = Arc::new(Mutex::new(None));
    let answer = Arc::clone(&read);
    fixture
        .app
        .run_ai_work_now(AiWork::new(AiWorkKey::Learning, move |_, run| {
            *answer.lock().unwrap() = Some(run.wait(async {
                tokio::task::yield_now().await;
                7
            }));
        }));
    assert_eq!(*read.lock().unwrap(), Some(7));
}

#[test]
fn work_with_no_host_installed_is_dropped() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = crate::tests_fakes::app(
        vec![crate::tests_fakes::account("acct-1", FakeProvider::new())],
        &surfaces,
    );
    let ran = Arc::new(Mutex::new(Vec::new()));
    assert_eq!(
        app.queue_ai_work(noting(AiWorkKey::Learning, "learn", &ran)),
        AiQueued::NoHost
    );
    assert_eq!(app.ai_work(), []);
}
