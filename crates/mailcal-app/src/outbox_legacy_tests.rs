//! An outbox an earlier build wrote, before a message's metadata was one value: a queued send names
//! the keywords for its Sent copy as `sent_copy_keywords`, a bare list, and a queued draft save
//! names its keywords as `keywords`, a bare list. Both still load, and carry their keywords.
//!
//! A child of the Outbox edit tests. The store is a file, so a test can rewrite a payload into
//! the shape the earlier build stored, as a second process would.

use std::{path::Path, sync::Arc};

use engine_api::{
    AccountId, Draft, EmailAddress, Engine, MessageIdHeader, MessageMetadata, PendingOpKind,
    TimeZoneId,
};
use mailcal_account::drafted_reply_keyword;
use serde_json::Value;

use super::{
    super::{SilentObserver, SubmitProvider, submit_account},
    edit, outbox_holding, save, staging,
};
use crate::{App, CompositionId, DraftStatus, Telemetry, TimeZoneInit};

/// A one-account app over `provider`, its store at `path`.
fn app_at(path: &Path, provider: SubmitProvider) -> Arc<App<SubmitProvider>> {
    Arc::new(App::new(
        Engine::open(path).unwrap(),
        vec![submit_account(provider)],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(SilentObserver),
        Telemetry::off(),
    ))
}

/// Rewrites the payload of `op` in the store at `path` with `rewrite`.
fn rewrite_payload(path: &Path, op: u64, rewrite: impl FnOnce(&mut Value)) {
    let store = rusqlite::Connection::open(path).unwrap();
    let op = i64::try_from(op).unwrap();
    let payload: String = store
        .query_row(
            "SELECT payload FROM pending_op_payload WHERE op_id = ?1",
            [op],
            |row| row.get(0),
        )
        .unwrap();
    let mut payload: Value = serde_json::from_str(&payload).unwrap();
    rewrite(&mut payload);
    store
        .execute(
            "UPDATE pending_op_payload SET payload = ?1 WHERE op_id = ?2",
            (payload.to_string(), op),
        )
        .unwrap();
}

/// Moves the metadata under `field` to `legacy`, as the bare keyword list an earlier build
/// stored there.
fn as_keyword_list(payload: &mut Value, field: &str, legacy: &str) {
    let fields = payload.as_object_mut().unwrap();
    let metadata = fields.remove(field).expect("the metadata is stored");
    fields.insert(legacy.to_owned(), metadata["keywords"].clone());
}

fn marked() -> MessageMetadata {
    MessageMetadata::default().with_keyword(drafted_reply_keyword())
}

#[tokio::test(start_paused = true)]
async fn a_queued_send_stored_with_a_keyword_list_is_edited_as_a_drafted_reply() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("store.sqlite");
    let provider = SubmitProvider::offline();
    let saved = provider.draft_metadata();
    let app = app_at(&path, provider);
    let draft = Draft::new(
        MessageIdHeader::new("drafted-reply@allodia.local").unwrap(),
        EmailAddress::new("me@allodia.local"),
        vec![EmailAddress::new("you@test.local")],
        "Re: Plans",
        "Friday works.",
    )
    .with_sent_copy_keyword(drafted_reply_keyword());
    app.send_draft(&AccountId::try_from("acct-1").unwrap(), &draft, None)
        .await;
    let row = outbox_holding(&app, 1).await.remove(0);
    rewrite_payload(&path, row.op, |payload| {
        as_keyword_list(payload, "sent_copy_metadata", "sent_copy_keywords");
        assert_eq!(
            payload["sent_copy_keywords"],
            serde_json::json!([drafted_reply_keyword().as_str()])
        );
    });

    app.dispatch(edit(&row, &staging(&scratch))).await;
    let request = app.compose_request().expect("the composer is offered");
    assert!(app.composition_drafted(&CompositionId::new(&request.composition).unwrap()));
    assert_eq!(*saved.lock().unwrap(), [marked()], "the draft it became");
}

#[tokio::test]
async fn a_queued_save_stored_with_a_keyword_list_is_stored_with_them() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("store.sqlite");
    let provider = SubmitProvider::saving_nothing_for(1);
    let saved = provider.draft_metadata();
    let app = app_at(&path, provider);
    let account = AccountId::try_from("acct-1").unwrap();
    app.dispatch(save("compose-1", "on a train")).await;
    let composition = CompositionId::new("compose-1").unwrap();
    assert_eq!(app.draft_status(&composition), DraftStatus::Queued);
    let queued = app
        .engine
        .outbox(&account)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.kind == Some(PendingOpKind::MailDraftPut))
        .expect("the save is queued");
    rewrite_payload(&path, queued.id.get(), |payload| {
        payload["metadata"] = serde_json::to_value(marked()).unwrap();
        as_keyword_list(payload, "metadata", "keywords");
    });

    app.flush_outboxes().await;

    assert_eq!(
        *saved.lock().unwrap(),
        [marked()],
        "stored with its keyword"
    );
}
