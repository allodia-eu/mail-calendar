//! The writing-style half of a pass, against a real app booted from a data directory, a transport
//! that never opens a socket and a bookkeeping store in memory.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
};

use allodia_license::{
    AccountService, Method, Request, Response, SyncedStyle, Transport, fingerprint,
};
use mailcal_account::{StoredWritingStyle, WritingStyleId, WritingStyles};
use mailcal_ai::{LanguageStyle, StyleGuide};
use serde_json::{Value, json};

use crate::{
    LogLevel, MailcalApp,
    allodia_pass::Pass,
    allodia_sync::AllodiaStyleConflict,
    sync_state::{StoredSyncState, SyncBookkeeping, SyncStateError, SyncStateStore},
    tests::{ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle},
};

/// A transport that answers from a script, in order, and records what it was asked.
struct Scripted {
    answers: Mutex<Vec<Response>>,
    seen: Mutex<Vec<Request>>,
}

impl Scripted {
    fn new(answers: &[(u16, &str)]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .map(|(status, body)| Response {
                        status: *status,
                        body: (*body).to_owned(),
                    })
                    .collect(),
            ),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
}

impl Transport for Scripted {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.lock().unwrap().push(request.clone());
        let mut answers = self.answers.lock().unwrap();
        if answers.is_empty() {
            return Err("the script ran out of answers".to_owned());
        }
        Ok(answers.remove(0))
    }
}

#[derive(Default)]
struct Prefs(Mutex<Option<String>>);

impl SyncStateStore for Prefs {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, blob: String) -> Result<(), SyncStateError> {
        *self.0.lock().unwrap() = Some(blob);
        Ok(())
    }
}

/// A passage of the person's own mail, which must never reach the service.
const PASSAGE: &str = "Beste Anna, de offerte volgt morgen voor twaalf uur.";

fn guide(notes: &str) -> StyleGuide {
    let mut guide = StyleGuide::new();
    guide.languages.insert(
        "nl".to_owned(),
        LanguageStyle {
            typical_words: 40,
            ..LanguageStyle::default()
        },
    );
    guide.notes = notes.to_owned();
    guide
}

fn style(name: &str, notes: &str) -> SyncedStyle {
    SyncedStyle {
        name: name.to_owned(),
        guide: guide(notes),
    }
}

/// A device whose library holds `styles`, each with a passage, as an earlier launch left it, and
/// which is signed in with `grant` when one is given.
fn device(
    name: &str,
    styles: &[(&str, &SyncedStyle)],
    grant: Option<&str>,
) -> (Arc<MailcalApp>, PathBuf) {
    let data_dir = crate::tests::temp_data_dir(name);
    std::fs::create_dir_all(&data_dir).unwrap();
    let mut library = WritingStyles::default();
    for (id, synced) in styles {
        library.insert(
            WritingStyleId::new(*id).unwrap(),
            StoredWritingStyle {
                name: synced.name.clone(),
                source: "acct-1".to_owned(),
                guide_json: serde_json::to_string(&synced.guide).unwrap(),
                exemplars_json: json!({ "schema_version": 1, "languages": { "nl": [PASSAGE] } })
                    .to_string(),
            },
        );
    }
    mailcal_account::save_writing_styles(mailcal_account::writing_styles_path(&data_dir), &library)
        .unwrap();
    if grant.is_some() {
        store_a_fresh_entitlement(&data_dir);
    }
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        grant.map(str::to_owned).into_iter().collect(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("an app with no mail accounts boots");
    (app, data_dir)
}

/// What a previous launch learned about the plan, fresh enough that this launch asks nothing.
fn store_a_fresh_entitlement(data_dir: &Path) {
    let stored = allodia_license::Stored {
        answer: allodia_license::Answer {
            entitlement: allodia_license::Entitlement::free(),
            refresh_after_seconds: 86_400,
        },
        fetched_at: time::OffsetDateTime::now_utc().unix_timestamp(),
    };
    let mut prefs = mailcal_account::Preferences::default();
    prefs.ai.entitlement_answer = Some(serde_json::to_string(&stored).unwrap());
    mailcal_account::save_preferences(mailcal_account::preferences_path(data_dir), &prefs).unwrap();
}

fn book() -> SyncBookkeeping {
    SyncBookkeeping::load(Box::new(Prefs::default())).unwrap()
}

/// This device in step with the service's record `id` at `version`.
fn in_step(book: &SyncBookkeeping, style_id: &str, id: &str, version: u64, base: &SyncedStyle) {
    book.set_style(
        style_id,
        StoredSyncState {
            id: id.to_owned(),
            version,
            fingerprint: fingerprint(base),
        },
    )
    .unwrap();
}

fn record(id: &str, version: u64, style: &SyncedStyle) -> Value {
    json!({ "id": id, "version": version, "style": style, "updatedAt": "2026-09-23T10:00:00Z" })
}

fn listing(styles: &[Value], deleted: &[(&str, u64)]) -> String {
    let deleted: Vec<Value> = deleted
        .iter()
        .map(|(id, version)| json!({ "id": id, "version": version, "deletedAt": "2026-09-22T08:00:00Z" }))
        .collect();
    json!({ "styles": styles, "deleted": deleted, "syncedAt": "2026-09-23T10:05:00Z" }).to_string()
}

fn sync(
    app: &MailcalApp,
    transport: &Scripted,
    book: &SyncBookkeeping,
) -> Vec<AllodiaStyleConflict> {
    let service = AccountService::new("https://mailcal.example.com");
    app.sync_writing_styles(&Pass {
        service: &service,
        transport,
        token: "an-access-token",
        bookkeeping: book,
    })
}

/// The stored passages of style `id`, as the library file holds them.
fn passages_of(data_dir: &Path, id: &str) -> Value {
    let library =
        mailcal_account::load_writing_styles(mailcal_account::writing_styles_path(data_dir));
    let stored = library.get(&WritingStyleId::new(id).unwrap()).unwrap();
    serde_json::from_str(&stored.exemplars_json).unwrap()
}

/// Every key starting `exemplar`, at any depth: what the service refuses a payload for.
fn exemplar_keys(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map
            .iter()
            .flat_map(|(key, inner)| {
                let mut found = exemplar_keys(inner);
                if key.to_lowercase().starts_with("exemplar") {
                    found.push(key.clone());
                }
                found
            })
            .collect(),
        Value::Array(items) => items.iter().flat_map(exemplar_keys).collect(),
        _ => Vec::new(),
    }
}

fn body_of(request: &Request) -> Value {
    serde_json::from_str(request.body.as_deref().unwrap()).unwrap()
}

#[test]
fn a_style_the_service_has_never_seen_is_uploaded_without_its_passages() {
    let work = style("Work", "Short.");
    let (app, data_dir) = device("style-upload", &[("s1", &work)], None);
    let stored = record("rec-1", 1, &work).to_string();
    let transport = Scripted::new(&[(200, &listing(&[], &[])), (200, &stored)]);
    let book = book();

    assert!(sync(&app, &transport, &book).is_empty());

    let sent = transport.requests();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].method, Method::Get);
    assert!(
        sent[0].url.ends_with("/api/v1/writing-styles"),
        "{}",
        sent[0].url
    );
    assert_eq!(sent[1].method, Method::Post);
    assert!(sent[1].idempotency_key.is_some());
    let body = body_of(&sent[1]);
    assert_eq!(body["style"]["name"], "Work");
    assert_eq!(body["style"]["guide"]["notes"], "Short.");
    assert!(exemplar_keys(&body).is_empty(), "{body}");
    assert!(!sent[1].body.as_deref().unwrap().contains(PASSAGE));
    let entry = book.style("s1").expect("the device now knows the record");
    assert_eq!((entry.id.as_str(), entry.version), ("rec-1", 1));
    assert_eq!(entry.fingerprint, fingerprint(&work));
    assert_eq!(book.pending_style_create_key("s1"), None);
    assert_eq!(passages_of(&data_dir, "s1")["languages"]["nl"][0], PASSAGE);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The second device. The style arrives as a name and a guide, and its passages come from this
/// device's own sent mail; this one holds none, so it has none, and no request is made for them.
#[test]
fn a_style_from_another_device_arrives_with_passages_from_this_device_only() {
    let (app, data_dir) = device("style-arrival", &[], None);
    let personal = style("Personal", "Warm.");
    let transport = Scripted::new(&[(200, &listing(&[record("rec-9", 2, &personal)], &[]))]);
    let book = book();

    assert!(sync(&app, &transport, &book).is_empty());

    assert_eq!(transport.requests().len(), 1, "a read, and nothing written");
    let here = app.app.syncable_writing_styles();
    assert_eq!(here.len(), 1);
    assert_eq!(here[0].name, "Personal");
    assert_eq!(here[0].guide, personal.guide);
    assert_eq!(passages_of(&data_dir, &here[0].id)["languages"], json!({}));
    let entry = book.style(&here[0].id).expect("the arrival is claimed");
    assert_eq!((entry.id.as_str(), entry.version), ("rec-9", 2));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_style_changed_only_elsewhere_is_applied_here_and_its_passages_stay() {
    let work = style("Work", "Short.");
    let (app, data_dir) = device("style-update", &[("s1", &work)], None);
    let book = book();
    in_step(&book, "s1", "rec-1", 1, &work);
    let office = style("Office", "Shorter.");
    let transport = Scripted::new(&[(200, &listing(&[record("rec-1", 2, &office)], &[]))]);

    assert!(sync(&app, &transport, &book).is_empty());

    assert_eq!(transport.requests().len(), 1);
    let here = app.app.syncable_writing_styles();
    assert_eq!(
        (here[0].name.as_str(), here[0].guide.notes.as_str()),
        ("Office", "Shorter.")
    );
    assert_eq!(passages_of(&data_dir, "s1")["languages"]["nl"][0], PASSAGE);
    assert_eq!(book.style("s1").unwrap().version, 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_style_changed_only_here_is_pushed_at_the_version_it_read() {
    let before = style("Work", "Short.");
    let after = style("Work", "Short, and never before nine.");
    let (app, data_dir) = device("style-push", &[("s1", &after)], None);
    let book = book();
    in_step(&book, "s1", "rec-1", 3, &before);
    let stored = record("rec-1", 4, &after).to_string();
    let transport = Scripted::new(&[
        (200, &listing(&[record("rec-1", 3, &before)], &[])),
        (200, &stored),
    ]);

    assert!(sync(&app, &transport, &book).is_empty());

    let sent = transport.requests();
    assert_eq!(sent[1].method, Method::Put);
    assert!(
        sent[1].url.ends_with("/writing-styles/rec-1"),
        "{}",
        sent[1].url
    );
    let body = body_of(&sent[1]);
    assert_eq!(body["version"], 3);
    assert!(exemplar_keys(&body).is_empty());
    assert_eq!(book.style("s1").unwrap().version, 4);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn a_style_changed_on_both_sides_is_reported_by_name_and_nothing_is_applied() {
    let base = style("Work", "Short.");
    let (app, data_dir) = device(
        "style-conflict",
        &[("s1", &style("Work (laptop)", "Short."))],
        None,
    );
    let book = book();
    in_step(&book, "s1", "rec-1", 3, &base);
    let transport = Scripted::new(&[(
        200,
        &listing(&[record("rec-1", 5, &style("Work (phone)", "Short."))], &[]),
    )]);

    let conflicts = sync(&app, &transport, &book);

    assert_eq!(
        conflicts,
        [AllodiaStyleConflict {
            style_id: "s1".to_owned(),
            name: "Work (laptop)".to_owned(),
        }]
    );
    assert_eq!(
        transport.requests().len(),
        1,
        "neither side is written over"
    );
    assert_eq!(app.app.syncable_writing_styles()[0].name, "Work (laptop)");
    assert_eq!(book.style("s1").unwrap().version, 3);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[path = "allodia_style_pass_removal_tests.rs"]
mod removal;
