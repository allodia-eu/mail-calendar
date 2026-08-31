//! One connection's traffic, and which era each request on it is speaking.
//!
//! # This server is dual-era, and the era is decided per request
//!
//! `2026-07-28` deleted the `initialize` handshake. A **modern** request carries its protocol
//! version in `_meta` and is answered on its own; a **legacy** request (`2025-11-25` and earlier)
//! belongs to a session a handshake opened. The specification's compatibility matrix is blunt
//! about what happens to a server that implements only one of them: a modern client against a
//! legacy server is listed as *"Fails"*, not as degrades. So both are implemented here, and
//! `modern` holds the newer half.
//!
//! The discriminator is **the method, then the metadata** (`modern::era_of`), and it needs no
//! per-connection state, which is the property that makes dual-era cheap rather than a second
//! server. Nothing in the legacy answer depends on which legacy revision was negotiated (see
//! `server_info`), so there is no negotiated version to remember, and a client may interleave
//! eras on one socket without confusing anything.
//!
//! # The legacy handshake counter-offers; it does not refuse
//!
//! MCP's `protocolVersion` is the one thing on this wire that genuinely churns, and the spec says
//! exactly what to do about it: *"If the server supports the requested protocol version, it MUST
//! respond with the same version. Otherwise, the server MUST respond with another protocol
//! version it supports … If the client does not support the version in the server's response, it
//! SHOULD disconnect."*
//!
//! So an unknown version is **not** an error. We answer with our own newest, log it, and let the
//! client decide whether it can live with that, which is where the decision belongs, because the
//! client is the only party that knows what else it speaks.
//!
//! The property worth protecting is *never claim to speak a version we do not*: a session that
//! negotiates something we half-implement works for three calls and then goes subtly wrong at a
//! layer nobody watches. Refusing an unknown version outright would protect that property, but it
//! also breaks **every** client on the day a new revision ships, before anyone has read it.
//! Counter-offering protects the same property; we only ever name a version from our own list;
//! while leaving the client a way through.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::{
    branding,
    jsonrpc::{self, INTERNAL_ERROR, Request, Response, RpcError},
    modern,
    policy::Budget,
    tools::{self, ToolContext, ToolFailure},
};

/// The **modern** revisions this server implements, newest first: no handshake, protocol version
/// and client capabilities on every request, `server/discover` instead of `initialize`.
///
/// Kept apart from [`LEGACY_PROTOCOL_VERSIONS`] rather than merged into one list, because the two
/// are not interchangeable and one place reads the list to *answer* with a version: the legacy
/// counter-offer below. Merged, that counter-offer would hand `2026-07-28` to a client that only
/// speaks the handshake: a version it cannot use, in reply to the one message that proves it
/// cannot. The split makes that unrepresentable rather than a rule to remember.
pub const MODERN_PROTOCOL_VERSIONS: &[&str] = &["2026-07-28"];

/// The **legacy** revisions this server implements, newest first; those that open with an
/// `initialize` handshake.
///
/// **One entry, and keeping it that way is the rule.** A revision in this list is a promise
/// measured in years: it is cheap to add; every legacy revision shares this server's framing and
/// its `tools/list` / `tools/call` shapes, and expensive to withdraw, because withdrawing one is
/// a client that stops working. A list that grows on cheapness only ever grows.
///
/// So an older revision earns a place here by a client that needs it, never by costing little.
/// Until then the floor holds, and a handshake below it is **counter-offered**
/// [`LATEST_LEGACY_PROTOCOL_VERSION`] rather than refused: a legacy revision's differences are
/// additive, so such a client usually speaks the floor too, and one that cannot disconnects
/// knowing exactly what this server does speak. What it never gets is silence, or a version this
/// server does not implement.
pub const LEGACY_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25"];

/// Every revision this server implements, newest first; what `server/discover` advertises and
/// what an [`UNSUPPORTED_PROTOCOL_VERSION`](crate::jsonrpc::UNSUPPORTED_PROTOCOL_VERSION) error
/// offers to retry from.
///
/// Deliberately **not** the source of the legacy counter-offer; see [`MODERN_PROTOCOL_VERSIONS`].
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2026-07-28", "2025-11-25"];

/// The newest **legacy** revision this server implements; what it offers a client whose
/// handshake names a version it does not recognise.
pub const LATEST_LEGACY_PROTOCOL_VERSION: &str = LEGACY_PROTOCOL_VERSIONS[0];

/// Serves one connection to completion: reads newline-framed requests, answers each, and returns
/// when the peer closes or the stream errors.
///
/// Notifications are **never** answered; MCP sends `notifications/initialized` immediately after
/// the handshake, and a response to it is a protocol violation some clients treat as fatal.
pub async fn serve<S>(ctx: ToolContext, stream: S)
where
    S: tokio::io::AsyncRead + AsyncWrite + Unpin + Send,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    let mut budget = Budget::new();
    let mut calls = 0_u64;
    loop {
        // A clean close, or a read error, either way this connection is over.
        let Ok(Some(line)) = lines.next_line().await else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = handle_line(&ctx, &mut budget, &line).await else {
            continue;
        };
        calls += 1;
        if writer.write_all(response.frame().as_bytes()).await.is_err() {
            break;
        }
        if writer.flush().await.is_err() {
            break;
        }
    }
    // Counts only; never a method name's arguments, a query, or an address.
    log::info!("mcp: connection closed after {calls} answered request(s)");
}

/// Handles one received line, returning the response to write, or `None` for a notification.
async fn handle_line(ctx: &ToolContext, budget: &mut Budget, line: &str) -> Option<Response> {
    let request = match jsonrpc::parse(line) {
        Ok(request) => request,
        // A parse failure has no id to echo, so the spec's `null` is used.
        Err(error) => return Some(Response::err(Value::Null, error)),
    };
    if request.is_notification() {
        log::debug!("mcp: notification {}", request.method);
        return None;
    }
    let id = request.id.clone().unwrap_or(Value::Null);
    Some(match dispatch(ctx, budget, &request).await {
        Ok(result) => Response::ok(id, result),
        Err(error) => Response::err(id, error),
    })
}

/// Routes one request to its era, and within that era to its method.
async fn dispatch(
    ctx: &ToolContext,
    budget: &mut Budget,
    request: &Request,
) -> Result<Value, RpcError> {
    match modern::era_of(&request.method, request.params.as_ref()) {
        modern::Era::Modern => modern::dispatch(ctx, budget, request).await,
        modern::Era::Legacy => legacy_dispatch(ctx, budget, request).await,
    }
}

/// Routes one request under the handshake-based revisions.
///
/// Byte-for-byte what this server answered before it was dual-era, and kept that way on purpose:
/// the modern envelope is added only on the modern path, so every legacy assertion in the suite
/// still describes the wire an old client sees, and a regression there cannot hide behind a new
/// field.
async fn legacy_dispatch(
    ctx: &ToolContext,
    budget: &mut Budget,
    request: &Request,
) -> Result<Value, RpcError> {
    match request.method.as_str() {
        "initialize" => initialize(request.params.as_ref()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_listing(ctx) })),
        "tools/call" => call_tool(ctx, budget, request.params.as_ref()).await,
        other => Err(RpcError::method_not_found(other)),
    }
}

/// The tool listing as `tools/list` returns it, in either era.
pub(crate) fn tool_listing(ctx: &ToolContext) -> Vec<Value> {
    tools::listing(&ctx.config())
        .iter()
        .map(tools::Tool::to_json)
        .collect()
}

/// The legacy handshake: agree a protocol version, and say what this server offers.
fn initialize(params: Option<&Value>) -> Result<Value, RpcError> {
    let requested = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("initialize requires a protocolVersion"))?;
    // Only the LEGACY list. A client that reached this method proved it speaks the handshake, so
    // counter-offering a modern revision would answer "I cannot do that" with "then do this other
    // thing you also cannot do".
    let negotiated = if LEGACY_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        // Counter-offer, as the spec requires, and log it, because "the assistant connected but
        // behaves oddly" is otherwise an unanswerable support question. The client disconnects if
        // it cannot live with our answer, that decision is its to make, not ours.
        log::warn!(
            "mcp: client asked for protocol version {requested}, which this build does not know; \
             offering {LATEST_LEGACY_PROTOCOL_VERSION}",
        );
        LATEST_LEGACY_PROTOCOL_VERSION
    };
    Ok(json!({
        "protocolVersion": negotiated,
        "capabilities": capabilities(),
        "serverInfo": server_info(),
    }))
}

/// What this server offers, in both eras: tools, and nothing else.
///
/// `listChanged: false` is honest rather than modest: the set does change, when the user edits
/// Settings, but nothing here pushes a notification when it does. What that costs a modern client
/// is spelled out at `modern::TOOLS_LIST`.
pub(crate) fn capabilities() -> Value {
    json!({ "tools": { "listChanged": false } })
}

/// How this server introduces itself (`branding`): the spec's `Implementation`.
///
/// The richer fields (`title`, `description`, `icons`, `websiteUrl`) arrived in `2025-11-25`,
/// which is now the legacy floor ([`LEGACY_PROTOCOL_VERSIONS`]), so every revision this server
/// implements defines all of them and there is nothing to gate them behind.
///
/// Sent where identity is asked for **once**: the legacy handshake, and `server/discover` (which
/// a client caches for an hour). Not on every modern result; see [`server_identity`] for why.
pub(crate) fn server_info() -> Value {
    json!({
        "name": branding::SERVER_NAME,
        "title": branding::SERVER_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
        "description": branding::SERVER_DESCRIPTION,
        "websiteUrl": branding::SERVER_WEBSITE,
        "icons": [{
            "src": branding::icon_data_uri(),
            "mimeType": "image/png",
            "sizes": ["128x128"],
        }],
    })
}

/// How this server names itself on **every** modern result; name, title and version, and
/// nothing else.
///
/// `2026-07-28` moved server identity from one handshake into every result's `_meta`, and that
/// changes what may go in it. [`server_info`]'s inlined 128×128 PNG is right for a handshake
/// answered once per session and for a `server/discover` a client caches for an hour; repeated on
/// every response it is **24 kB of icon attached to a two-line tool result**, measured over the
/// real socket. A `list_accounts` answer that is 99% logo is not a cosmetic problem on a channel
/// an assistant calls in a loop.
///
/// The specification's own example of this field is a bare name and version, and its note says
/// the field is for display, logging and debugging: a client that wants to *draw* this server
/// asks `server/discover`, which is exactly where the icon still is.
pub(crate) fn server_identity() -> Value {
    json!({
        "name": branding::SERVER_NAME,
        "title": branding::SERVER_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// `tools/call`: run the named tool and shape its result.
///
/// Identical in both eras: the tool surface, the policy controls and the refusal semantics are
/// what this server is, and none of them moved. Only the envelope around the result differs.
pub(crate) async fn call_tool(
    ctx: &ToolContext,
    budget: &mut Budget,
    params: Option<&Value>,
) -> Result<Value, RpcError> {
    let name = params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("tools/call requires a tool name"))?;
    let args = params
        .and_then(|params| params.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let started = std::time::Instant::now();
    let outcome = tools::call(ctx, budget, name, args).await;
    // The tool's name, whether it worked, and how long it took. Never its arguments.
    log::info!(
        "mcp: tools/call {name} -> {} in {}ms",
        if outcome.is_ok() { "ok" } else { "error" },
        started.elapsed().as_millis(),
    );
    match outcome {
        Ok(structured) => Ok(success(&structured)),
        // A refusal or a bad argument is a TOOL error, not a protocol error: the model should
        // see it, relay it, and possibly try something else. A protocol error would instead look
        // to most clients like the server is broken.
        Err(ToolFailure::Refused(message) | ToolFailure::BadArgs(message)) => {
            Ok(tool_error(&message))
        }
        Err(ToolFailure::Unknown(name)) => Err(RpcError::method_not_found(&format!("tool {name}"))),
        Err(ToolFailure::Internal(detail)) => Err(RpcError::new(INTERNAL_ERROR, detail)),
    }
}

/// A successful tool result: the structured value, plus its JSON as the text fallback older
/// clients render. Emitting both is free and makes a result machine-checkable rather than prose
/// a model has to parse back.
fn success(structured: &Value) -> Value {
    let text = serde_json::to_string_pretty(structured).unwrap_or_else(|_| structured.to_string());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false,
    })
}

/// A tool-level failure, in the shape MCP defines for one.
fn tool_error(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

/// Builds a session context over the running app and the **live** configuration.
#[must_use]
pub fn context(
    backend: Arc<dyn crate::backend::MailBackend>,
    config: crate::tools::SharedConfig,
) -> ToolContext {
    ToolContext::new(backend, config)
}
