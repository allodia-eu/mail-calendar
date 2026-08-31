//! The JSON-RPC 2.0 wire format MCP speaks, and its newline framing.
//!
//! Hand-rolled rather than taken from the official Rust SDK. The protocol actually needed here
//! is a few hundred lines (no sampling, no roots, no elicitation, no HTTP transport) while the
//! SDK is a large tree with its own release cadence, and every dependency in this workspace
//! carries a justification comment it would struggle to earn. Three of those four omissions the
//! specification has since **deprecated**, so the surface this server declined to implement is
//! the surface that is going away.
//!
//! The risk that trade accepts is spec churn, and `2026-07-28` is what churn looks like when it
//! arrives: it deletes the handshake this module was written around. What absorbed it was small,
//! because the parts of that revision that are large are the parts this server has no transport
//! for, statelessness over HTTP, session headers, stream resumability. What reaches a local
//! socket carrying twelve tools is an envelope and one new method (`modern`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parse error: the payload was not valid JSON.
pub const PARSE_ERROR: i32 = -32_700;
/// The payload was valid JSON but not a valid Request object.
pub const INVALID_REQUEST: i32 = -32_600;
/// The method does not exist.
pub const METHOD_NOT_FOUND: i32 = -32_601;
/// The parameters were missing, malformed, or out of range.
pub const INVALID_PARAMS: i32 = -32_602;
/// An error inside the server.
pub const INTERNAL_ERROR: i32 = -32_603;
/// An application-level failure: the reserved implementation-defined range. This is the code
/// the **relay** answers with when the app is not running; the server itself does not raise it.
///
/// `2026-07-28` closed this sub-range (`-32000` to `-32019`) to new allocations and reserved
/// `-32020` to `-32099` for the specification. Codes already in use are **grandfathered**, which
/// is why this one stays where it is rather than moving and breaking every relay a user has
/// already configured.
pub const SERVER_ERROR: i32 = -32_000;

/// The request named a protocol version this server does not implement (`2026-07-28`).
///
/// Carries `data.supported` and `data.requested`, which is the whole point of it. A client that
/// receives this learns two things at once: the server is **modern**, so it must not fall back to
/// the `initialize` handshake, and here is the list to retry from. Answering a version mismatch
/// with a generic code instead would send a dual-era client down the legacy path: the spec is
/// explicit that fallback is keyed on *not* recognising the error, so a wrong code here is not a
/// cosmetic problem.
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32_022;

/// One incoming message. A **notification** is a request with no `id`, and MCP sends two of them
/// (`notifications/initialized`, `notifications/cancelled`), so the distinction is load-bearing:
/// answering a notification is a protocol violation that some clients treat as fatal.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// The protocol tag. Present on every valid message; checked, not assumed.
    #[serde(default)]
    pub jsonrpc: String,
    /// The request id, absent on a notification. A JSON value because the spec allows a string
    /// or a number and a client may use either.
    #[serde(default)]
    pub id: Option<Value>,
    /// The method name.
    pub method: String,
    /// The parameters, absent for a parameterless method.
    #[serde(default)]
    pub params: Option<Value>,
}

impl Request {
    /// Whether this is a notification (no `id`), which must never be answered.
    #[must_use]
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// One outgoing response; exactly one of `result` or `error`, never both.
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// The id of the request being answered, echoed verbatim. `null` for an error raised before
    /// an id could be read (a parse failure), as the spec requires.
    pub id: Value,
    /// The successful result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    /// A successful response to `id`.
    #[must_use]
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    /// A failed response to `id`.
    #[must_use]
    pub fn err(id: Value, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }

    /// This response as one newline-terminated frame.
    ///
    /// Serialization cannot fail for this type (every field is a plain value), but a `.expect()`
    /// in a socket loop would take the app down with it, so a failure degrades to a
    /// minimal hand-built error frame instead.
    #[must_use]
    pub fn frame(&self) -> String {
        match serde_json::to_string(self) {
            Ok(body) => format!("{body}\n"),
            Err(_) => {
                "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"response could not be serialized\"}}\n".to_owned()
            }
        }
    }
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    /// The error code; one of the constants in this module.
    pub code: i32,
    /// A short, **non-localised** diagnostic. Never carries mail content, an address, or a
    /// search query: this string is written to the client's own log file.
    pub message: String,
    /// Structured detail an error is defined to carry, omitted entirely when there is none.
    ///
    /// Under the same rule as `message`, and more sharply: whatever goes here is protocol
    /// metadata a client may log verbatim. A version list belongs here, nothing about the user's
    /// mail does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Builds an error with `code` and `message`, and no structured detail.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// The same error, carrying `data`.
    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// The method is not one this server implements.
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("unknown method: {method}"))
    }

    /// The parameters did not match what the method's schema documents.
    #[must_use]
    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, detail)
    }
}

/// Parses one received line into a [`Request`].
///
/// # Errors
///
/// [`PARSE_ERROR`] for malformed JSON, [`INVALID_REQUEST`] for JSON that is not a request object
/// or that carries the wrong protocol version.
pub fn parse(line: &str) -> Result<Request, RpcError> {
    let request: Request = serde_json::from_str(line)
        .map_err(|err| RpcError::new(PARSE_ERROR, format!("invalid JSON: {err}")))?;
    if request.jsonrpc != "2.0" {
        return Err(RpcError::new(
            INVALID_REQUEST,
            "jsonrpc must be exactly \"2.0\"",
        ));
    }
    Ok(request)
}
