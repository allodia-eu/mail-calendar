//! The writing assistant as a host reaches it: offered in a development build, the own endpoint's
//! form saved through `dispatch` with its key in the keystore, the key taken out of the stored
//! configs at launch, and work queued onto a thread of its own.

use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use mailcal_app::ai::{AiQueued, AiWork, AiWorkKey};

use crate::{
    AiEndpointAddressGap, AiEndpointDeclarationGap, AiEndpointForm, AiEndpointIntent,
    AiEndpointResult, AiIntent, Intent, JurisdictionClass, JurisdictionMode, LogLevel, MailcalApp,
    Surface,
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

fn app(
    name: &str,
    configs: Vec<String>,
) -> (
    Arc<MailcalApp>,
    Arc<RecordingCredentialStore>,
    mpsc::Receiver<Surface>,
) {
    let (tx, rx) = mpsc::channel();
    let dir = crate::tests::temp_data_dir(name);
    let store = Arc::new(RecordingCredentialStore::default());
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&store))),
    )
    .expect("the app boots");
    (app, store, rx)
}

/// The form once a change after `serial` has ended.
fn form_after(app: &MailcalApp, serial: u64) -> AiEndpointForm {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let form = app.ai_endpoint_form().expect("offered");
        if form.outcome.is_some_and(|outcome| outcome.serial > serial)
            && !form.save.running
            && !form.remove.running
        {
            return form;
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn save(app: &MailcalApp, address: &str, declared: JurisdictionClass, key: Option<&str>) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: address.to_owned(),
                model: "local-model".to_owned(),
                declared: Some(declared),
                key: key.map(str::to_owned),
            },
        },
    });
}

/// Waits until every one of the assistant's records has been signalled: the page, the composer's
/// Draft a reply, the endpoint's form and Settings, each pulled again on its own surface.
pub(crate) fn every_ai_surface_signalled(rx: &mpsc::Receiver<Surface>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = [false; 4];
    while seen.contains(&false) {
        let left = deadline.saturating_duration_since(Instant::now());
        let surface = rx
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("not every surface was signalled: {seen:?}"));
        match surface {
            Surface::Settings => seen[0] = true,
            Surface::WritingStyle => seen[1] = true,
            Surface::ReplyDraft => seen[2] = true,
            Surface::AiEndpoint => seen[3] = true,
            _ => {}
        }
    }
}

/// A test build is a development build, so the assistant is offered without an entitlement.
#[test]
fn a_development_build_offers_the_form_with_nothing_set_up() {
    let (app, ..) = app("ai-offered", Vec::new());
    assert!(app.ai_offer());
    let form = app.ai_endpoint_form().expect("offered");
    assert_eq!(form.address, "");
    assert!(!form.key_kept);
    assert!(form.save.enabled);
    assert!(!form.remove.enabled, "nothing to remove");
    assert_eq!(form.gaps.address, Some(AiEndpointAddressGap::Missing));
}

#[test]
fn a_save_dispatched_keeps_the_key_in_the_keystore_and_says_so_on_the_form() {
    let (app, store, rx) = app("ai-save", Vec::new());
    save(
        &app,
        "https://models.example.eu/v1",
        JurisdictionClass::EuNative,
        Some(" sk-test "),
    );
    let form = form_after(&app, 0);

    assert_eq!(
        form.outcome.map(|outcome| outcome.result),
        Some(AiEndpointResult::Saved)
    );
    assert_eq!(form.address, "https://models.example.eu/v1");
    assert_eq!(form.declared, Some(JurisdictionClass::EuNative));
    assert!(form.key_kept);
    assert!(form.remove.enabled);
    let persisted = store.persisted.lock().unwrap().clone();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].0, "ai-endpoint");
    assert!(persisted[0].1.contains("sk-test") && !persisted[0].1.contains(" sk-test "));
    // Where a request goes moved, so the page and every composer read theirs again too.
    every_ai_surface_signalled(&rx);

    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Remove,
        },
    });
    let form = form_after(&app, 1);
    assert_eq!(
        form.outcome.map(|outcome| outcome.result),
        Some(AiEndpointResult::Removed)
    );
    assert_eq!(*store.deleted.lock().unwrap(), ["ai-endpoint"]);
}

/// The function the form asks as the person types is the one the save refuses by.
#[test]
fn the_gap_the_form_shows_is_the_one_the_save_refuses_by() {
    let (app, store, _) = app("ai-gap", Vec::new());
    let gaps = app.ai_endpoint_gap(
        "http://models.example.eu/v1".to_owned(),
        String::new(),
        Some(JurisdictionClass::NonEu),
    );
    assert_eq!(gaps.address, Some(AiEndpointAddressGap::NeedsHttps));
    assert!(gaps.model_missing);
    assert_eq!(
        gaps.declaration,
        Some(AiEndpointDeclarationGap::Refused {
            mode: JurisdictionMode::EuNative,
            class: JurisdictionClass::NonEu,
        })
    );
    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: "http://models.example.eu/v1".to_owned(),
                model: String::new(),
                declared: Some(JurisdictionClass::NonEu),
                key: Some("sk".to_owned()),
            },
        },
    });
    let form = form_after(&app, 0);
    assert_eq!(
        form.outcome.map(|outcome| outcome.result),
        Some(AiEndpointResult::Refused { gaps })
    );
    assert_eq!(
        form.address, "http://models.example.eu/v1",
        "the form keeps the edit"
    );
    assert!(store.persisted.lock().unwrap().is_empty());
}

/// The key a previous launch stored is taken out before any mail parser sees it, and kept.
#[test]
fn the_stored_key_is_taken_out_at_launch_and_kept() {
    let (app, ..) = app(
        "ai-launch",
        vec!["[ai_endpoint]\nkey = \"sk-stored\"\n".to_owned()],
    );
    assert!(
        app.account_connect_errors.lock().unwrap().is_empty(),
        "the key is not an account"
    );
    assert!(app.ai_endpoint_form().expect("offered").key_kept);
}

#[test]
fn work_queued_runs_on_a_thread_of_its_own() {
    let (app, ..) = app("ai-work", Vec::new());
    let (done, finished) = mpsc::channel();
    let ran = Arc::new(Mutex::new(false));
    let seen = Arc::clone(&ran);
    let queued = app.queue_ai_work(AiWork::new(AiWorkKey::Learning, move |_, run| {
        *seen.lock().unwrap() = !run.cancelled();
        done.send(()).unwrap();
    }));
    assert_eq!(queued, AiQueued::Run);
    finished.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(*ran.lock().unwrap());
}
