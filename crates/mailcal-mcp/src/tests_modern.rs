//! The **modern** (`2026-07-28`) surface, over the same duplex the legacy suite uses.
//!
//! Two properties are worth more than the individual assertions and are what this file is really
//! for.
//!
//! **The eras must not drift in what they expose.** They share `GOLDEN_TOOLS` with the legacy
//! suite rather than keeping a second list, so a tool that appears on one path and not the other
//! fails here, and the thing the two paths would have disagreed about is the security surface.
//!
//! **The era is decided per request, with no memory.** Several cases below interleave the two on
//! one connection, because a per-connection discriminator would pass every single-era test in the
//! suite and then break the first client that probes with `server/discover` before falling back.

use serde_json::{Value, json};

use crate::{
    jsonrpc::{INVALID_PARAMS, METHOD_NOT_FOUND, UNSUPPORTED_PROTOCOL_VERSION},
    modern::{META_CLIENT_CAPABILITIES, META_PROTOCOL_VERSION, META_SERVER_INFO},
    session::{LEGACY_PROTOCOL_VERSIONS, MODERN_PROTOCOL_VERSIONS, SUPPORTED_PROTOCOL_VERSIONS},
    tests_protocol::{GOLDEN_TOOLS, exposing_work, request, round_trip, session_with},
};

/// The `_meta` a conforming modern client puts on every request.
fn meta() -> Value {
    json!({
        META_PROTOCOL_VERSION: MODERN_PROTOCOL_VERSIONS[0],
        META_CLIENT_CAPABILITIES: {},
        "io.modelcontextprotocol/clientInfo": {"name": "test-client", "version": "1.0.0"},
    })
}

/// A modern request: `params` with the required metadata merged in.
fn modern(id: i32, method: &str, params: &Value) -> Value {
    let mut params = params.clone();
    params["_meta"] = meta();
    request(id, method, &params)
}

#[test]
fn the_strings_the_specification_chose_are_pinned_to_their_literals() {
    // Every other assertion in this file names these through their constants, which proves
    // nothing about the constants themselves: rename `META_PROTOCOL_VERSION`'s *value* and the
    // whole suite stays green while no real client is ever recognised as modern again. The same
    // applies to the error code: a wrong number there is not a wrong number, it is a dual-era
    // client concluding this server is legacy and never probing again.
    //
    // So this one case is written the way it has to be: the literals, copied from the
    // specification, compared against what the code uses.
    assert_eq!(
        META_PROTOCOL_VERSION,
        "io.modelcontextprotocol/protocolVersion"
    );
    assert_eq!(
        META_CLIENT_CAPABILITIES,
        "io.modelcontextprotocol/clientCapabilities"
    );
    assert_eq!(META_SERVER_INFO, "io.modelcontextprotocol/serverInfo");
    assert_eq!(UNSUPPORTED_PROTOCOL_VERSION, -32_022);
    assert_eq!(MODERN_PROTOCOL_VERSIONS, ["2026-07-28"]);
}

#[test]
fn the_advertised_version_list_is_exactly_the_two_era_lists() {
    // Three constants, and `server/discover` publishes the third. Left to drift, the failure is
    // silent in the worst direction: this server would advertise a revision it does not implement
    // (or hide one it does) to every client that probes it.
    let joined: Vec<&str> = MODERN_PROTOCOL_VERSIONS
        .iter()
        .chain(LEGACY_PROTOCOL_VERSIONS.iter())
        .copied()
        .collect();
    assert_eq!(SUPPORTED_PROTOCOL_VERSIONS, joined.as_slice());
    assert!(
        !MODERN_PROTOCOL_VERSIONS
            .iter()
            .any(|version| LEGACY_PROTOCOL_VERSIONS.contains(version)),
        "a revision cannot be in both eras: one has a handshake and the other does not",
    );
}

#[tokio::test]
async fn server_discover_answers_the_probe_that_identifies_this_server_as_modern() {
    // On stdio there is no status code to key a fallback on, so this response is the *whole*
    // mechanism by which a dual-era client learns not to drop to `initialize`. The specification
    // makes the method a MUST for exactly that reason.
    let mut client = session_with(exposing_work());
    let response = round_trip(&mut client, &modern(1, "server/discover", &json!({}))).await;

    assert!(response.get("error").is_none(), "not an error: {response}");
    let result = &response["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(
        result["supportedVersions"],
        json!(SUPPORTED_PROTOCOL_VERSIONS)
    );
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    assert_eq!(
        result["_meta"][META_SERVER_INFO]["name"],
        crate::SERVER_NAME
    );
    // Build constants that cannot change under a live process, and that say nothing about the
    // user: so a real TTL and a shareable scope are both honest here.
    assert!(result["ttlMs"].as_u64().unwrap() > 0);
    assert_eq!(result["cacheScope"], "public");
}

#[tokio::test]
async fn a_modern_tools_list_offers_exactly_the_tools_the_handshake_offers() {
    // The drift guard. Two dispatch paths reach one listing, and the day they stop doing so is
    // the day one era quietly gains or loses a tool.
    let mut client = session_with(exposing_work());
    let response = round_trip(&mut client, &modern(1, "tools/list", &json!({}))).await;

    let names: Vec<&str> = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, GOLDEN_TOOLS);
}

#[tokio::test]
async fn a_tool_listing_is_never_cached_because_nothing_here_can_invalidate_one() {
    // `listChanged: false` plus a positive TTL would be a promise this server cannot keep: the
    // set changes when the user edits Settings and nothing pushes an invalidation. Concretely,
    // the bug a non-zero TTL buys is "I turned direct send on and the tool is still missing".
    //
    // `private` is the other half, and it is not about freshness: the listing differs per user,
    // so a shared cache would offer one user's `send_message` to a user who declined it.
    let mut client = session_with(exposing_work());
    let response = round_trip(&mut client, &modern(1, "tools/list", &json!({}))).await;

    assert_eq!(response["result"]["ttlMs"], 0);
    assert_eq!(response["result"]["cacheScope"], "private");
}

#[tokio::test]
async fn a_modern_tool_call_carries_the_envelope_but_no_caching_hints() {
    // A tool call is not in the revision's cacheable set, and it must not appear to be: these
    // results depend on the user's mail, and a client that cached one would answer a later
    // question from a stale mailbox.
    let mut client = session_with(exposing_work());
    let response = round_trip(
        &mut client,
        &modern(1, "tools/call", &json!({"name": "list_accounts"})),
    )
    .await;

    let result = &response["result"];
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["accounts"][0]["account"],
        "work"
    );
    assert_eq!(
        result["_meta"][META_SERVER_INFO]["name"],
        crate::SERVER_NAME
    );
    assert!(
        result.get("ttlMs").is_none(),
        "a tool call is not cacheable"
    );
    assert!(result.get("cacheScope").is_none());
}

#[tokio::test]
async fn only_the_discovery_call_pays_for_the_icon() {
    // Found by running it, not by reading it. This revision moved server identity out of a
    // once-per-session handshake and onto EVERY result's `_meta`, and the identity this server
    // had was built for the handshake; it inlines a 128x128 PNG. Measured over the real socket
    // before the split: a `list_accounts` answer was 24 kB, essentially all logo, on a channel an
    // assistant calls in a loop.
    //
    // So `server/discover`, asked once and cached for an hour, and the call a client makes in
    // order to draw this server; keeps the icon, and nothing else carries it.
    let mut client = session_with(exposing_work());

    let discover = round_trip(&mut client, &modern(1, "server/discover", &json!({}))).await;
    let identity = &discover["result"]["_meta"][META_SERVER_INFO];
    assert!(identity["icons"][0]["src"].is_string(), "{identity}");

    for (id, method, params) in [
        (2, "tools/list", json!({})),
        (3, "tools/call", json!({"name": "list_accounts"})),
    ] {
        let response = round_trip(&mut client, &modern(id, method, &params)).await;
        let identity = &response["result"]["_meta"][META_SERVER_INFO];
        assert_eq!(identity["name"], crate::SERVER_NAME, "still identified");
        assert!(
            identity.get("icons").is_none(),
            "{method} carries the icon on every response: {identity}",
        );
        // The envelope, not the payload; `tools/list` is legitimately ~19 kB of tool schemas,
        // and bounding the whole response would be measuring the wrong thing. What regressed was
        // `_meta`, so that is what is pinned: ~120 bytes here against ~24 kB with the icon.
        assert!(
            serde_json::to_string(identity).unwrap().len() < 512,
            "{method}'s identity is too big to ride on every response: {identity}",
        );
    }
}

#[tokio::test]
async fn an_unsupported_modern_version_is_refused_with_the_code_that_says_we_are_modern() {
    // The single most consequential code in this file. The specification's stdio fallback is
    // keyed on *not recognising* the error, so answering this with a generic `-32601`/`-32602`
    // would tell a dual-era client that this server is legacy, and it would drop to the
    // handshake permanently, caching that conclusion "for the lifetime of the server process".
    let mut client = session_with(exposing_work());
    let mut params = json!({});
    params["_meta"] = json!({
        META_PROTOCOL_VERSION: "2099-01-01",
        META_CLIENT_CAPABILITIES: {},
    });
    let response = round_trip(&mut client, &request(1, "tools/list", &params)).await;

    let error = &response["error"];
    assert_eq!(error["code"], UNSUPPORTED_PROTOCOL_VERSION);
    assert_eq!(error["data"]["requested"], "2099-01-01");
    assert_eq!(
        error["data"]["supported"],
        json!(SUPPORTED_PROTOCOL_VERSIONS)
    );
}

#[tokio::test]
async fn a_probe_that_forgets_its_metadata_is_told_which_field_is_missing() {
    // `server/discover` exists in no legacy revision, so it is modern whatever it carries, and
    // the answer is the specification's `-32602` for a malformed request naming the field;
    // not "unknown method", which would send a client author looking for a typo.
    //
    // Either way a dual-era client falls back to `initialize` and gets served, because neither
    // code is a recognised modern error. The difference is entirely in what the author reads.
    let mut client = session_with(exposing_work());
    let response = round_trip(&mut client, &request(1, "server/discover", &json!({}))).await;

    assert_eq!(response["error"]["code"], INVALID_PARAMS);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains(META_PROTOCOL_VERSION),
        "the error names the missing field: {}",
        response["error"]["message"],
    );
}

#[tokio::test]
async fn the_required_client_capabilities_field_is_checked_even_though_it_is_never_read() {
    // This server initiates nothing back at a client, so it needs no capability from one. The
    // field is still required by the revision, and a server that silently accepts a malformed
    // request teaches a client author that their client is conforming when it is not.
    let mut client = session_with(exposing_work());
    let mut params = json!({});
    params["_meta"] = json!({ META_PROTOCOL_VERSION: MODERN_PROTOCOL_VERSIONS[0] });
    let response = round_trip(&mut client, &request(1, "tools/list", &params)).await;

    assert_eq!(response["error"]["code"], INVALID_PARAMS);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains(META_CLIENT_CAPABILITIES),
    );
}

#[tokio::test]
async fn ping_is_answered_under_the_handshake_and_gone_under_the_revision_that_removed_it() {
    // `ping` was removed by `2026-07-28`. Keeping it for legacy clients costs nothing; answering
    // it to a modern one would be claiming a method the revision deleted.
    let mut client = session_with(exposing_work());

    let modern_ping = round_trip(&mut client, &modern(1, "ping", &json!({}))).await;
    assert_eq!(modern_ping["error"]["code"], METHOD_NOT_FOUND);

    let legacy_ping = round_trip(&mut client, &request(2, "ping", &json!({}))).await;
    assert_eq!(legacy_ping["result"], json!({}));
}

#[tokio::test]
async fn the_eras_interleave_on_one_connection_because_neither_leaves_state_behind() {
    // What a real dual-era client does on its first connection: probe with `server/discover`,
    // and, on a server it decided was legacy, or simply because a user reconfigured it;
    // handshake afterwards. A discriminator that remembered the first request's era would serve
    // the rest of this connection in the wrong one, and every single-era test above would still
    // pass over it.
    let mut client = session_with(exposing_work());

    let probe = round_trip(&mut client, &modern(1, "server/discover", &json!({}))).await;
    assert_eq!(probe["result"]["resultType"], "complete");

    let handshake = round_trip(
        &mut client,
        &request(
            2,
            "initialize",
            &json!({"protocolVersion": LEGACY_PROTOCOL_VERSIONS[0]}),
        ),
    )
    .await;
    assert_eq!(
        handshake["result"]["protocolVersion"],
        LEGACY_PROTOCOL_VERSIONS[0],
    );

    let modern_call = round_trip(&mut client, &modern(3, "tools/list", &json!({}))).await;
    assert_eq!(modern_call["result"]["resultType"], "complete");

    let legacy_call = round_trip(&mut client, &request(4, "tools/list", &json!({}))).await;
    assert_eq!(legacy_call["result"]["resultType"], Value::Null);
}

#[tokio::test]
async fn a_legacy_result_gains_none_of_the_modern_envelope() {
    // The isolation that lets the legacy suite go on describing the wire an old client sees. A
    // stray `resultType` would not break those clients (an additive key never does) but it
    // would mean the two paths had been collapsed, and the next change would have nowhere to
    // fail.
    let mut client = session_with(exposing_work());
    let response = round_trip(&mut client, &request(1, "tools/list", &json!({}))).await;

    let result = &response["result"];
    assert!(result.get("resultType").is_none());
    assert!(result.get("_meta").is_none());
    assert!(result.get("ttlMs").is_none());
    assert!(result.get("cacheScope").is_none());
    assert!(result["tools"].is_array(), "still a real listing: {result}");
}

/// Both eras, driven over the same connection, must agree on what an unexposed account is.
#[tokio::test]
async fn the_account_allow_list_binds_the_modern_path_too() {
    // The policy controls are the reason this server is allowed to exist at all, and a second
    // dispatch path is exactly where one gets forgotten. `tools/call` is shared between the eras
    // precisely so this cannot diverge: this asserts that it stayed shared.
    let mut client = session_with(crate::McpConfig::default());
    let response = round_trip(
        &mut client,
        &modern(1, "tools/call", &json!({"name": "list_accounts"})),
    )
    .await;

    let accounts = &response["result"]["structuredContent"]["accounts"];
    assert_eq!(
        accounts.as_array().unwrap().len(),
        0,
        "the default configuration exposes no account, in either era",
    );
}
