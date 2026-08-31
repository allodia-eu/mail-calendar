//! The **legacy** JSON-RPC surface, driven over an in-memory duplex exactly as a socket would be.
//!
//! These run the real [`session::serve`] loop rather than calling `dispatch` directly, so the
//! framing, the notification rule, and the "one line in, one line out" discipline are all under
//! test: the three things a client breaks on and a unit test of the handler would miss.
//!
//! Every request here opens with `initialize` and carries no `_meta`, so every one of them takes
//! the handshake path. That is the point of leaving the file otherwise untouched now that the
//! server is dual-era: these assertions describe the wire an **old** client still sees, and they
//! would have to be edited for a modern field to appear on it. `tests_modern` is the other era.

use std::{collections::BTreeSet, sync::Arc};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

use crate::{
    config::McpConfig,
    session::{self, LEGACY_PROTOCOL_VERSIONS},
    tests_fake::FakeBackend,
};

/// The exact tool set a client is offered with direct send **off**.
///
/// A golden list, so adding, renaming or removing a tool is a deliberate edit to this test
/// rather than something that appears in a client's UI unnoticed. Shared with `tests_modern`
/// rather than copied, so the one list governs both eras: two lists would let the modern path
/// grow or lose a tool without the legacy assertion noticing, and what the eras disagreed about
/// would be the security surface.
pub(crate) const GOLDEN_TOOLS: &[&str] = &[
    "list_accounts",
    "list_folders",
    "list_messages",
    "search_messages",
    "get_message",
    "mark_read",
    "set_flagged",
    "archive_message",
    "move_to_trash",
    "mark_as_spam",
    "create_draft",
];

/// A live session over a duplex, with `work` exposed.
pub(crate) fn session_with(config: McpConfig) -> DuplexStream {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let (backend, _) = FakeBackend::new();
    let ctx = session::context(backend, Arc::new(std::sync::RwLock::new(Arc::new(config))));
    tokio::spawn(async move { session::serve(ctx, server).await });
    client
}

pub(crate) fn exposing_work() -> McpConfig {
    McpConfig {
        accounts: BTreeSet::from(["work".to_owned()]),
        ..McpConfig::default()
    }
}

/// Writes one request and reads one response line.
pub(crate) async fn round_trip(client: &mut DuplexStream, request: &Value) -> Value {
    let line = format!("{request}\n");
    client.write_all(line.as_bytes()).await.unwrap();
    client.flush().await.unwrap();
    let mut reader = BufReader::new(client);
    let mut response = String::new();
    reader.read_line(&mut response).await.unwrap();
    serde_json::from_str(&response).expect("the server answered with one JSON line")
}

pub(crate) fn request(id: i32, method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

#[tokio::test]
async fn the_happy_path_is_initialize_then_initialized_then_list_then_call() {
    let mut client = session_with(exposing_work());

    let init = round_trip(
        &mut client,
        &request(
            1,
            "initialize",
            &json!({"protocolVersion": LEGACY_PROTOCOL_VERSIONS[0]}),
        ),
    )
    .await;
    assert_eq!(
        init["result"]["protocolVersion"],
        LEGACY_PROTOCOL_VERSIONS[0]
    );
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        "allodia-mail-and-calendar"
    );

    // A notification must NOT be answered. Sending it and then immediately sending a real
    // request proves it: if the server replied to the notification, the next read would return
    // that reply instead of the tools/list result and this test would fail on the id.
    client
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await
        .unwrap();

    let listed = round_trip(&mut client, &request(2, "tools/list", &json!({}))).await;
    assert_eq!(listed["id"], 2, "the notification was not answered");
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, GOLDEN_TOOLS);

    let called = round_trip(
        &mut client,
        &request(3, "tools/call", &json!({"name": "list_accounts"})),
    )
    .await;
    assert_eq!(called["result"]["isError"], false);
    let accounts = &called["result"]["structuredContent"]["accounts"];
    assert_eq!(
        accounts.as_array().unwrap().len(),
        1,
        "only the exposed one"
    );
    assert_eq!(accounts[0]["account"], "work");
    assert!(
        called["result"]["content"][0]["text"].is_string(),
        "the text fallback is emitted alongside the structured result",
    );
}

#[tokio::test]
async fn an_unknown_protocol_version_is_counter_offered_our_latest_not_refused() {
    // What the spec requires: "Otherwise, the server MUST respond with another protocol version
    // it supports … If the client does not support the version in the server's response, it
    // SHOULD disconnect." The decision belongs to the client, which is the only party that knows
    // what else it speaks.
    //
    // This replaced a hard refusal, which was both non-conformant and the wrong trade: it broke
    // every client on the day a new revision shipped. The very first real client to connect asked
    // for a version newer than the list and was turned away.
    let mut client = session_with(exposing_work());
    let response = round_trip(
        &mut client,
        &request(1, "initialize", &json!({"protocolVersion": "2099-01-01"})),
    )
    .await;
    assert!(response.get("error").is_none(), "not an error: {response}");
    assert_eq!(
        response["result"]["protocolVersion"],
        crate::session::LATEST_LEGACY_PROTOCOL_VERSION,
    );
}

#[tokio::test]
async fn a_revision_below_the_floor_is_counter_offered_rather_than_echoed() {
    // What lets the supported list stay short: a client below the floor is not refused and not
    // met with silence, it is answered with the floor and decides for itself. A legacy revision's
    // differences are additive, so such a client usually speaks `2025-11-25` too; one that cannot
    // disconnects knowing exactly what this server does speak.
    //
    // The three below are the real MCP revisions a client is most likely to ask for from under
    // the floor, which is what makes them worth naming rather than probing with a made-up date.
    for below in ["2025-06-18", "2025-03-26", "2024-11-05"] {
        let mut client = session_with(exposing_work());
        let response = round_trip(
            &mut client,
            &request(1, "initialize", &json!({"protocolVersion": below})),
        )
        .await;
        assert!(response.get("error").is_none(), "not a refusal: {response}");
        assert_eq!(
            response["result"]["protocolVersion"],
            crate::session::LATEST_LEGACY_PROTOCOL_VERSION,
            "{below} should be counter-offered the floor, not echoed back",
        );
    }
}

#[tokio::test]
async fn a_handshake_is_never_counter_offered_a_modern_revision() {
    // The trap the split version lists exist to make unrepresentable. A client that sent
    // `initialize` proved it speaks the handshake and nothing else; answering it with
    // `2026-07-28` (which has no handshake) would reply to "I cannot do that" with "then do
    // this other thing you also cannot do". Merging the two lists into one is all it would take.
    for asked in ["2099-01-01", "2026-07-28"] {
        let mut client = session_with(exposing_work());
        let response = round_trip(
            &mut client,
            &request(1, "initialize", &json!({"protocolVersion": asked})),
        )
        .await;
        let answered = response["result"]["protocolVersion"].as_str().unwrap();
        assert!(
            LEGACY_PROTOCOL_VERSIONS.contains(&answered),
            "a handshake asking for {asked:?} was offered {answered:?}, which is not a revision \
             the handshake exists in",
        );
    }
}

#[tokio::test]
async fn the_server_never_claims_to_speak_a_version_it_does_not() {
    // The property the old refusal was protecting, kept. A session that negotiates something we
    // half-implement works for three calls and then goes subtly wrong at a layer nobody watches;
    // so whatever we answer with must come from our own list, never be echoed back unchecked.
    for asked in ["2099-01-01", "1.0.0", "", "banana"] {
        let mut client = session_with(exposing_work());
        let response = round_trip(
            &mut client,
            &request(1, "initialize", &json!({"protocolVersion": asked})),
        )
        .await;
        let answered = response["result"]["protocolVersion"].as_str().unwrap();
        assert!(
            LEGACY_PROTOCOL_VERSIONS.contains(&answered),
            "answered {answered:?} to a request for {asked:?}, which is not a version we implement",
        );
    }
}

#[tokio::test]
async fn every_version_we_advertise_is_echoed_back_unchanged() {
    // The other half of negotiation: a client asking for something we DO support must get that
    // exact version, not our newest, or an older client would be told to speak a revision it
    // cannot.
    for supported in LEGACY_PROTOCOL_VERSIONS {
        let mut client = session_with(exposing_work());
        let response = round_trip(
            &mut client,
            &request(1, "initialize", &json!({"protocolVersion": supported})),
        )
        .await;
        assert_eq!(response["result"]["protocolVersion"], *supported);
    }
}

#[tokio::test]
async fn malformed_and_unknown_input_map_onto_the_standard_codes() {
    let mut client = session_with(exposing_work());

    client.write_all(b"{not json at all\n").await.unwrap();
    let mut reader = BufReader::new(&mut client);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let parse_error: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(parse_error["error"]["code"], crate::jsonrpc::PARSE_ERROR);
    assert_eq!(parse_error["id"], Value::Null, "no id to echo");

    let unknown = round_trip(&mut client, &request(2, "does/not/exist", &json!({}))).await;
    assert_eq!(unknown["error"]["code"], crate::jsonrpc::METHOD_NOT_FOUND);

    let ping = round_trip(&mut client, &request(3, "ping", &json!({}))).await;
    assert_eq!(ping["result"], json!({}));
}

#[tokio::test]
async fn a_missing_required_argument_is_a_tool_error_the_model_can_read() {
    // Deliberately a tool error rather than a protocol error: the model should see it, relay it
    // and correct itself. A -32602 looks to most clients like the server is broken.
    let mut client = session_with(exposing_work());
    let response = round_trip(
        &mut client,
        &request(
            1,
            "tools/call",
            &json!({"name": "list_folders", "arguments": {}}),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("account"),
        "the message names the field that was missing",
    );
}

#[tokio::test]
async fn an_unknown_argument_is_refused_rather_than_silently_dropped() {
    // `deny_unknown_fields` is what makes the published schema honest: a typo'd argument that
    // was quietly ignored would have the tool answer a question nobody asked.
    let mut client = session_with(exposing_work());
    let response = round_trip(
        &mut client,
        &request(
            1,
            "tools/call",
            &json!({"name": "list_folders", "arguments": {"account": "work", "acount": "typo"}}),
        ),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn tools_call_on_a_name_that_is_not_listed_is_method_not_found() {
    let mut client = session_with(exposing_work());
    let response = round_trip(
        &mut client,
        &request(1, "tools/call", &json!({"name": "permanently_delete"})),
    )
    .await;
    assert_eq!(response["error"]["code"], crate::jsonrpc::METHOD_NOT_FOUND);
}

#[tokio::test]
async fn send_message_is_absent_from_the_listing_until_the_user_turns_it_on() {
    // Absent, not present-and-erroring. A tool a model can see is a tool it will try, and a
    // refusal it can retry differently reads as an obstacle rather than an answer.
    let mut off = session_with(exposing_work());
    let listed = round_trip(&mut off, &request(1, "tools/list", &json!({}))).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"send_message"));

    let mut on = session_with(McpConfig {
        allow_direct_send: true,
        ..exposing_work()
    });
    let listed = round_trip(&mut on, &request(1, "tools/list", &json!({}))).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.last(), Some(&"send_message"));
}

#[tokio::test]
async fn every_listed_tool_publishes_an_input_and_output_schema() {
    let mut client = session_with(McpConfig {
        allow_direct_send: true,
        ..exposing_work()
    });
    let listed = round_trip(&mut client, &request(1, "tools/list", &json!({}))).await;
    for tool in listed["result"]["tools"].as_array().unwrap() {
        let name = tool["name"].as_str().unwrap();
        assert_eq!(
            tool["inputSchema"]["type"], "object",
            "{name} publishes an object input schema",
        );
        assert!(
            tool["outputSchema"].is_object(),
            "{name} publishes an output schema",
        );
        assert!(
            tool["annotations"]["readOnlyHint"].is_boolean(),
            "{name} sets its annotations",
        );
        assert!(
            tool["inputSchema"]
                .get("$defs")
                .is_none_or(|defs| defs.as_object().is_none_or(serde_json::Map::is_empty)),
            "{name}'s schema is self-contained; client `$ref` support is uneven",
        );
    }
}
