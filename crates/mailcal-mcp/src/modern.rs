//! The `2026-07-28` revision: no handshake, metadata on every request, `server/discover`.
//!
//! # What this revision actually changed here
//!
//! Its headline is that MCP became stateless, and read cold the changelog looks like a rewrite:
//! sessions deleted, the handshake deleted, `ping` and `logging/setLevel` gone, roots, sampling
//! and logging deprecated, server-initiated requests replaced by a retry pattern, subscriptions
//! reshaped, stream resumability removed.
//!
//! Almost none of that reaches this server, and the reason is the shape it already had. The
//! deleted machinery is either **HTTP's** (session headers, resumability, status-code fallback;
//! there is no HTTP transport here) or a **feature this server deliberately never grew** (see
//! `tools`: no sampling, no roots, no elicitation, no subscriptions, no tasks). A local socket
//! offering twelve tools was already, in effect, stateless: nothing in an answer here depended on
//! anything but the request and the user's live configuration.
//!
//! So what is left is this module: read the per-request metadata, put four fields on the way out,
//! and answer one new method.
//!
//! # Statelessness, and the one place it is not free
//!
//! The revision says a server **MUST NOT** infer state from earlier requests on the same
//! connection. This server never did; `docs/mcp.md` records that the configuration is read live
//! on every call rather than captured per connection, precisely so that unticking an account
//! revokes an already-connected assistant. That decision was made for revocation and is what
//! makes this revision nearly free.
//!
//! The exception is [`TOOLS_LIST`], below: statelessness plus caching means a client may now hold
//! a tool list this server can no longer correct.

use serde_json::{Map, Value, json};

use crate::{
    jsonrpc::{Request, RpcError, UNSUPPORTED_PROTOCOL_VERSION},
    policy::Budget,
    session::{self, MODERN_PROTOCOL_VERSIONS, SUPPORTED_PROTOCOL_VERSIONS},
    tools::ToolContext,
};

/// `_meta` key: the protocol version a request is speaking. Required on every modern request,
/// and the field this module treats as the era discriminator.
pub(crate) const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";

/// `_meta` key: the client capabilities relevant to a request. Required on every modern request.
///
/// This server needs none of them (it initiates nothing back at the client) so the value is
/// never read. It is still *checked*, because the revision makes it required and a server that
/// quietly accepts a malformed request teaches a client author that their client is correct.
pub(crate) const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

/// `_meta` key: how this server identifies itself on every result it returns.
pub(crate) const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// Which revision family one request is speaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Era {
    /// `2026-07-28` and later: version and capabilities in `_meta`, no handshake, and results
    /// carrying `resultType`.
    Modern,
    /// `2025-11-25` and earlier: an `initialize` handshake, and bare results.
    Legacy,
}

/// Decides one request's era from its method, then its metadata.
///
/// Method first, because two methods settle it outright and settling it there produces a better
/// diagnostic than metadata alone could:
///
/// * `server/discover` exists in **no** legacy revision, so it is always a modern request; even
///   with no `_meta` at all. That is what turns a malformed probe into the specification's own
///   `-32602` ("a request missing any required field is malformed") instead of a bare "unknown
///   method", which names the missing field for whoever is writing the client.
/// * `initialize` exists in no modern revision, so it is always legacy, whatever it carries.
///
/// Everything else is decided by whether the request brought [`META_PROTOCOL_VERSION`]. There is
/// no per-connection memory in any of this, so a client may interleave the two eras on one
/// socket, which the revision's statelessness rule permits and a stateful discriminator would
/// have quietly broken.
pub(crate) fn era_of(method: &str, params: Option<&Value>) -> Era {
    match method {
        "server/discover" => Era::Modern,
        "initialize" => Era::Legacy,
        _ if meta(params).is_some_and(|meta| meta.get(META_PROTOCOL_VERSION).is_some()) => {
            Era::Modern
        }
        _ => Era::Legacy,
    }
}

/// How long a client may treat a result as fresh, and who may hold it.
#[derive(Debug, Clone, Copy)]
struct CacheHints {
    /// Freshness in milliseconds, `0` meaning "immediately stale".
    ttl_ms: u64,
    /// `"public"` (holds nothing about this user) or `"private"`.
    scope: &'static str,
}

/// `server/discover` answers with build constants: the version lists, the capability set and the
/// server's identity; none of which can change while the process this socket belongs to is
/// alive, and none of which say anything about the user. An hour, and shareable.
const DISCOVER: CacheHints = CacheHints {
    ttl_ms: 3_600_000,
    scope: "public",
};

/// `tools/list` is **uncacheable, and private**, and both halves are deliberate.
///
/// *Private*, because the listing is not the same for every user: `send_message` appears only for
/// someone who turned direct send on, so a shared cache would hand one user's answer to another
/// and offer a send tool the second user declined.
///
/// *Uncacheable*, because this server advertises `listChanged: false` and therefore sends nothing
/// when the set changes; while the set changes whenever the user touches Settings. A TTL is the
/// only bound on how long a stale listing survives, so anything above zero is a promise this
/// server cannot keep. Concretely: turn direct send on, and with a five-minute TTL the tool the
/// user just enabled stays invisible for five minutes.
///
/// The other direction is already safe, and it is worth being exact about why, because it is what
/// makes zero a freshness decision rather than a security one; `tools::call` re-reads the live
/// configuration on every call, so a client holding a listing from before the user revoked direct
/// send gets a refusal, not a send. The listing is a hint; enforcement never depended on it.
///
/// The alternative is to advertise `listChanged: true` and push an invalidation on every settings
/// change. That means plumbing configuration edits out to every open connection, which is real
/// work for a list that costs nothing to rebuild.
const TOOLS_LIST: CacheHints = CacheHints {
    ttl_ms: 0,
    scope: "private",
};

/// Routes one modern request, after checking the metadata the revision requires of all of them.
pub(crate) async fn dispatch(
    ctx: &ToolContext,
    budget: &mut Budget,
    request: &Request,
) -> Result<Value, RpcError> {
    check_meta(request.params.as_ref())?;
    match request.method.as_str() {
        "server/discover" => Ok(discover()),
        "tools/list" => Ok(complete(
            json!({ "tools": session::tool_listing(ctx) }),
            Some(TOOLS_LIST),
        )),
        "tools/call" => {
            let result = session::call_tool(ctx, budget, request.params.as_ref()).await?;
            Ok(complete(result, None))
        }
        // `ping` and `logging/setLevel` were removed by this revision, so they land here rather
        // than being answered: a modern client that calls one is owed the truth about it.
        other => Err(RpcError::method_not_found(other)),
    }
}

/// Validates the `_meta` fields the revision requires on every request.
///
/// The version is checked **before** the capabilities, so a client on the wrong revision gets the
/// error that tells it what to do next rather than a complaint about a field its revision may not
/// define.
fn check_meta(params: Option<&Value>) -> Result<(), RpcError> {
    let meta = meta(params);
    let Some(version) = meta
        .and_then(|meta| meta.get(META_PROTOCOL_VERSION))
        .and_then(Value::as_str)
    else {
        return Err(RpcError::invalid_params(format!(
            "a request must carry `{META_PROTOCOL_VERSION}` in `_meta`"
        )));
    };
    if !MODERN_PROTOCOL_VERSIONS.contains(&version) {
        // Never a generic code. The specification's stdio fallback is keyed on *not* recognizing
        // the error: anything else here tells a dual-era client this server is legacy, and it
        // would drop to the handshake instead of retrying from the list below.
        return Err(RpcError::new(
            UNSUPPORTED_PROTOCOL_VERSION,
            format!("this build does not implement protocol version {version}"),
        )
        .with_data(json!({
            "supported": SUPPORTED_PROTOCOL_VERSIONS,
            "requested": version,
        })));
    }
    if meta
        .and_then(|meta| meta.get(META_CLIENT_CAPABILITIES))
        .is_none()
    {
        return Err(RpcError::invalid_params(format!(
            "a request must carry `{META_CLIENT_CAPABILITIES}` in `_meta`"
        )));
    }
    Ok(())
}

/// `server/discover`: every revision this server implements, what it offers, and who it is.
///
/// Required of every server by this revision, and load-bearing beyond its own result: on stdio it
/// is the probe a dual-era client sends first, so answering it is what identifies this server as
/// modern at all.
/// The one modern response carrying the **full** identity, icon included. It is the call a client
/// makes in order to *draw* this server, and it caches the answer for an hour, which is what the
/// inlined PNG is affordable in and a per-result `_meta` is not.
fn discover() -> Value {
    complete_as(
        json!({
            "supportedVersions": SUPPORTED_PROTOCOL_VERSIONS,
            "capabilities": session::capabilities(),
        }),
        Some(DISCOVER),
        session::server_info(),
    )
}

/// Puts the modern envelope on a finished result: `resultType`, this server's identity, and the
/// caching hints for the operations that must carry them.
///
/// `body` is always a JSON object (every result this server builds is one) so the non-object
/// case needs no handling beyond skipping the fields. The shape that would then go out is the
/// legacy one, which the revision already tells clients to read as `"complete"`.
fn complete(body: Value, cache: Option<CacheHints>) -> Value {
    // The default is the lightweight identity, and it is a default rather than an argument so
    // that a result added later cannot quietly start shipping the icon on every response.
    complete_as(body, cache, session::server_identity())
}

/// [`complete`], for the one result that names this server in full.
fn complete_as(body: Value, cache: Option<CacheHints>, identity: Value) -> Value {
    let mut result = body;
    if let Some(object) = result.as_object_mut() {
        object.insert("resultType".to_owned(), json!("complete"));
        let mut meta = Map::new();
        meta.insert(META_SERVER_INFO.to_owned(), identity);
        object.insert("_meta".to_owned(), Value::Object(meta));
        if let Some(cache) = cache {
            object.insert("ttlMs".to_owned(), json!(cache.ttl_ms));
            object.insert("cacheScope".to_owned(), json!(cache.scope));
        }
    }
    result
}

/// A request's `_meta`, if it brought one.
fn meta(params: Option<&Value>) -> Option<&Value> {
    params.and_then(|params| params.get("_meta"))
}
