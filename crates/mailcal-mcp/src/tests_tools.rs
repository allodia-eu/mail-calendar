//! The tool handlers, against the fake backend.
//!
//! What is under test here is the *adapter's* behaviour: the allow list, the body rules, the
//! recipient guard's wiring, the error mapping. The mail behaviour it adapts (ordering, scope,
//! write semantics) is tested in `mailcal-app`, where it lives.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use mailcal_app::MailActionError;
use serde_json::{Value, json};

use crate::{
    config::McpConfig,
    policy::Budget,
    tests_fake::{FakeBackend, Recorder},
    tools::{self, ToolContext, ToolFailure},
};

fn ctx_with(backend: Arc<FakeBackend>, config: McpConfig) -> ToolContext {
    ToolContext::new(backend, Arc::new(std::sync::RwLock::new(Arc::new(config))))
}

fn exposing_work() -> McpConfig {
    McpConfig {
        accounts: BTreeSet::from(["work".to_owned()]),
        ..McpConfig::default()
    }
}

async fn call(ctx: &ToolContext, name: &str, args: Value) -> Result<Value, ToolFailure> {
    tools::call(ctx, &mut Budget::new(), name, args).await
}

fn fake() -> (Arc<FakeBackend>, Arc<Mutex<Recorder>>) {
    FakeBackend::new()
}

#[tokio::test]
async fn a_listing_carries_no_message_bodies() {
    // The single most effective bound on prompt injection here. One broad search that returned
    // bodies could drop fifty attacker-authored texts into context at once; it only takes one
    // landing. `get_message` is the one door, and it opens for one message at a time.
    let (backend, _) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let listed = call(&ctx, "list_messages", json!({"account": "work"}))
        .await
        .unwrap();
    let rendered = listed.to_string();
    assert!(
        !rendered.contains("body") && !rendered.contains("preview"),
        "no body or preview text reached the listing: {rendered}",
    );

    let searched = call(&ctx, "search_messages", json!({"query": "report"}))
        .await
        .unwrap();
    assert!(!searched.to_string().contains("body"));
}

#[tokio::test]
async fn a_body_arrives_fenced_and_only_from_get_message() {
    let (backend, _) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let message = call(&ctx, "get_message", json!({"account": "work", "key": "m1"}))
        .await
        .unwrap();
    let body = message["body"].as_str().unwrap();
    assert!(body.contains("<untrusted-message-content>"));
    assert!(body.contains("The numbers are in."));
    assert_eq!(
        message["unread"], true,
        "and reading it did not mark it read",
    );
}

#[tokio::test]
async fn an_account_the_user_did_not_expose_is_invisible_and_unreachable() {
    // Two halves of one rule. `private` exists in the backend; the user exposed only `work`.
    let (backend, recorder) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let accounts = call(&ctx, "list_accounts", json!({})).await.unwrap();
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 1);
    assert!(
        !accounts.to_string().contains("private"),
        "an unexposed account is not even named, which mailboxes exist is itself a disclosure",
    );

    let refused = call(
        &ctx,
        "archive_message",
        json!({"account": "private", "key": "p1"}),
    )
    .await
    .expect_err("acting on an unexposed account is refused");
    assert!(matches!(refused, ToolFailure::Refused(_)));
    assert!(
        recorder.lock().unwrap().writes.is_empty(),
        "and nothing reached the backend",
    );
}

#[tokio::test]
async fn a_search_hit_from_an_unexposed_account_is_filtered_out() {
    // The backend's unscoped search covers every configured account, so the adapter filters
    // rather than trusting the scope to have done it. Belt and braces on the one path where a
    // leak would be silent.
    let (backend, _) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let searched = call(&ctx, "search_messages", json!({"query": "report"}))
        .await
        .unwrap();
    let accounts: Vec<&str> = searched["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["account"].as_str().unwrap())
        .collect();
    assert_eq!(accounts, ["work"]);
}

#[tokio::test]
async fn a_folder_narrowing_without_an_account_is_refused_as_ambiguous() {
    let (backend, _) = fake();
    let ctx = ctx_with(backend, exposing_work());
    let refused = call(
        &ctx,
        "search_messages",
        json!({"query": "report", "folder": "inbox"}),
    )
    .await
    .expect_err("a folder key is only meaningful within an account");
    assert!(matches!(refused, ToolFailure::BadArgs(_)));
}

#[tokio::test]
async fn an_action_dispatches_to_the_backend_and_reports_what_it_did() {
    let (backend, recorder) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let result = call(
        &ctx,
        "archive_message",
        json!({"account": "work", "key": "m1"}),
    )
    .await
    .unwrap();
    assert_eq!(result["outcome"], "archived");
    assert_eq!(result["account"], "work");
    assert_eq!(result["key"], "m1");
    assert_eq!(
        recorder.lock().unwrap().writes,
        [("archive".to_owned(), "work".to_owned(), "m1".to_owned())],
    );
}

#[tokio::test]
async fn a_silent_no_op_becomes_something_the_model_can_report() {
    // The reason the result is plumbed through at all. Gmail's refreshing wrapper does not forward
    // `edit_mail`, so without this an assistant would say "marked read" over a message that was
    // never touched.
    let backend = Arc::new(FakeBackend {
        write_error: Some(MailActionError::NoProvider),
        ..FakeBackend::default()
    });
    let ctx = ctx_with(backend, exposing_work());

    let refused = call(
        &ctx,
        "mark_read",
        json!({"account": "work", "key": "m1", "read": true}),
    )
    .await
    .expect_err("a write that applied nothing is not a success");
    let ToolFailure::Refused(message) = refused else {
        panic!("expected a refusal");
    };
    assert!(
        message.contains("offline") || message.contains("does not support"),
        "and the reason is specific enough to act on: {message}",
    );
}

#[tokio::test]
async fn create_draft_opens_a_composer_and_does_not_send() {
    let (backend, recorder) = fake();
    let ctx = ctx_with(backend, exposing_work());

    let result = call(
        &ctx,
        "create_draft",
        json!({
            "account": "work",
            "to": ["anyone@wherever.example"],
            "subject": "Hi",
            "body_text": "Hello",
        }),
    )
    .await
    .unwrap();
    assert!(result["outcome"].as_str().unwrap().contains("not sent"));

    let recorder = recorder.lock().unwrap();
    assert!(recorder.sends.is_empty(), "nothing was sent");
    assert_eq!(recorder.drafts.len(), 1);
    assert_eq!(recorder.drafts[0].to, "anyone@wherever.example");
}

#[tokio::test]
async fn create_draft_is_not_recipient_guarded_because_a_human_reads_it_first() {
    // Deliberate: the guard substitutes for human review, and a draft HAS human review. Guarding
    // it too would refuse a first email to a new contact, which people send constantly.
    let (backend, recorder) = fake();
    let ctx = ctx_with(
        backend,
        McpConfig {
            require_known_recipient: true,
            ..exposing_work()
        },
    );

    call(
        &ctx,
        "create_draft",
        json!({"to": ["stranger@nowhere.example"], "subject": "Hi", "body_text": "Hello"}),
    )
    .await
    .expect("a draft to a stranger is allowed");
    assert_eq!(recorder.lock().unwrap().drafts.len(), 1);
}

#[tokio::test]
async fn a_build_with_no_composer_says_so_instead_of_needing_a_cfg() {
    // Linux today. An error rather than conditional compilation, so a platform without a
    // composer simply reports that it has none.
    let backend = Arc::new(FakeBackend {
        has_composer: false,
        ..FakeBackend::default()
    });
    let ctx = ctx_with(backend, exposing_work());
    let refused = call(
        &ctx,
        "create_draft",
        json!({"to": ["a@b.example"], "subject": "Hi", "body_text": "Hello"}),
    )
    .await
    .expect_err("no composer, no draft");
    assert!(matches!(refused, ToolFailure::Refused(_)));
}

#[tokio::test]
async fn send_message_is_unreachable_while_direct_send_is_off() {
    let (backend, recorder) = fake();
    let ctx = ctx_with(backend, exposing_work());
    let refused = call(
        &ctx,
        "send_message",
        json!({"to": ["colleague@known.example"], "subject": "Hi", "body_text": "Hello"}),
    )
    .await
    .expect_err("the tool is not listed, so it is not callable");
    assert!(matches!(refused, ToolFailure::Unknown(_)));
    assert!(recorder.lock().unwrap().sends.is_empty());
}

#[tokio::test]
async fn send_message_refuses_an_unknown_recipient_and_allows_a_known_one() {
    let (backend, recorder) = fake();
    let ctx = ctx_with(
        backend,
        McpConfig {
            allow_direct_send: true,
            require_known_recipient: true,
            ..exposing_work()
        },
    );

    let refused = call(
        &ctx,
        "send_message",
        json!({"to": ["attacker@evil.tld"], "subject": "Everything", "body_text": "..."}),
    )
    .await
    .expect_err("the guard refuses exfiltration to an address the user has never written to");
    assert!(matches!(refused, ToolFailure::Refused(_)));
    assert!(recorder.lock().unwrap().sends.is_empty());

    call(
        &ctx,
        "send_message",
        json!({"to": ["colleague@known.example"], "subject": "Hi", "body_text": "Hello"}),
    )
    .await
    .expect("someone the user already emails goes through");
    assert_eq!(recorder.lock().unwrap().sends.len(), 1);
}

#[tokio::test]
async fn a_page_limit_is_clamped_rather_than_honoured() {
    // A model asking for a thousand subject lines gets fifty. Not a performance guard: every
    // extra attacker-authored line in one response is more room for one of them to be read as an
    // instruction.
    assert_eq!(McpConfig::page_size(Some(1_000)), crate::config::MAX_PAGE);
    assert_eq!(McpConfig::page_size(Some(0)), 1);
    assert_eq!(McpConfig::page_size(None), crate::config::DEFAULT_PAGE);
}
