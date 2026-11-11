//! Reading a request, routing it to its verb, and the shape of every reply.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::{actions, capture, query, settle, tree};

/// A verb's reply, or the failure that ends it. The code is what a caller branches on.
pub(super) type Answer = Result<Value, Failure>;

#[derive(Debug)]
pub(super) struct Failure {
    pub(super) message: String,
    pub(super) code: &'static str,
    pub(super) extra: Map<String, Value>,
}

impl Failure {
    pub(super) fn new(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            message: message.into(),
            code,
            extra: Map::new(),
        }
    }

    pub(super) fn with(mut self, key: &str, value: Value) -> Self {
        self.extra.insert(key.to_owned(), value);
        self
    }
}

pub(super) fn failure(message: &str, code: &'static str) -> Value {
    json!({ "ok": false, "error": message, "code": code })
}

/// Every verb, its arguments and what it answers: the reply to `verbs`, and the protocol's
/// documentation. A filter is the argument set `query` takes.
const VERBS: &[(&str, &str)] = &[
    ("verbs", "{} -> every verb and its arguments"),
    (
        "ping",
        "{} -> {pid, surfaces}: surfaces counts the core's change signals so far",
    ),
    (
        "windows",
        "{} -> {windows: [{h, type, title, width, height, active, modal}]}",
    ),
    (
        "query",
        "{text?, textLike?, type?, name?, icon?, role?, window?, under?, hidden?, sensitive?, has?, \
         index?} -> {elements: [node]}. text matches a label, title, tooltip or value exactly; \
         textLike is a case-insensitive substring; type is a GType name and matches subclasses \
         (GtkButton finds a GtkToggleButton); role is an accessible role (button, row, \
         text-box); window is part of a window title; has is a filter a descendant must match; \
         hidden includes widgets that are not on screen.",
    ),
    (
        "wait",
        "{filter…, absent?, timeoutMs=10000} -> {elements, ms}. Polls until the filter matches \
         (or matches nothing, with absent).",
    ),
    (
        "read",
        "{h} -> node, with the full text of an editable or a text view",
    ),
    (
        "dump",
        "{under?, window?, hidden?, full?} -> {text}: the tree, indented. Compact unless full: \
         a container that carries nothing is folded into its children.",
    ),
    (
        "activate",
        "{h} -> {how}: what a click does, done at once. A button is clicked, a menu button \
         opens, a list row is chosen and activated, an expander row opens or closes.",
    ),
    (
        "set-text",
        "{h, text, activate?} -> {}: an entry, entry row or text view",
    ),
    (
        "toggle",
        "{h, active?} -> {active}: a check, toggle, switch or expander",
    ),
    (
        "select",
        "{h, index} -> {}: a drop-down, combo row or list box",
    ),
    ("focus", "{h} -> {focused}"),
    (
        "action",
        "{name, target?, h?} -> {}: activates a GTK action such as win.close or app.quit, \
         looked up from h (or the active window) outwards. target is GVariant text: 'text', \
         true, int32 3.",
    ),
    (
        "actions",
        "{} -> {actions}: the application's and the active window's actions",
    ),
    (
        "eval",
        "{h, script} -> {value}: runs JavaScript in a web view; value is its JSON",
    ),
    (
        "screenshot",
        "{path, h?} -> {path, width, height}: renders h (or the active window) to a PNG, at the \
         display's scale. A popover is a surface of its own: pass its h to capture it.",
    ),
    (
        "settle",
        "{quietMs=300, capMs=10000} -> {ms, settled}: waits until the tree and the core have \
         both been still for quietMs",
    ),
    ("quit", "{} -> {}: quits the app as its own Quit does"),
];

pub(super) async fn handle(line: &str) -> Value {
    let reply = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(request)) => route(request).await,
        Ok(_) => Err(Failure::new("a request is a JSON object", "bad-request")),
        Err(error) => Err(Failure::new(format!("not JSON: {error}"), "bad-request")),
    };
    match reply {
        Ok(Value::Object(mut fields)) => {
            fields.insert("ok".to_owned(), Value::Bool(true));
            Value::Object(fields)
        }
        Ok(other) => json!({ "ok": true, "value": other }),
        Err(failure) => {
            let mut fields = failure.extra;
            fields.insert("ok".to_owned(), Value::Bool(false));
            fields.insert("error".to_owned(), Value::String(failure.message));
            fields.insert("code".to_owned(), Value::String(failure.code.to_owned()));
            Value::Object(fields)
        }
    }
}

async fn route(mut request: Map<String, Value>) -> Answer {
    let Some(Value::String(verb)) = request.remove("verb") else {
        return Err(Failure::new(
            "a request names its verb: {\"verb\": \"verbs\"} lists them",
            "bad-request",
        ));
    };
    match verb.as_str() {
        "verbs" => no_args(&request).map(|()| {
            json!({ "verbs": VERBS.iter().map(|(verb, about)| json!({ "verb": verb, "about": about })).collect::<Vec<_>>() })
        }),
        "ping" => no_args(&request).map(|()| settle::ping()),
        "windows" => no_args(&request).map(|()| tree::windows()),
        "query" => query::query(&args(request)?),
        "wait" => settle::wait(settle::WaitArgs::parse(request)?).await,
        "read" => tree::read(&args(request)?),
        "dump" => tree::dump(&args(request)?),
        "activate" => actions::activate(&args(request)?),
        "set-text" => actions::set_text(&args(request)?),
        "toggle" => actions::toggle(&args(request)?),
        "select" => actions::select(&args(request)?),
        "focus" => actions::focus(&args(request)?),
        "action" => actions::action(&args(request)?),
        "actions" => no_args(&request).map(|()| actions::actions()),
        "eval" => actions::eval(args(request)?).await,
        "screenshot" => capture::screenshot(args(request)?).await,
        "settle" => settle::settle(args(request)?).await,
        "quit" => no_args(&request).map(|()| actions::quit()),
        other => Err(Failure::new(
            format!("no verb {other:?}: {{\"verb\": \"verbs\"}} lists them"),
            "unknown-verb",
        )),
    }
}

/// A verb's arguments, refusing a field it does not know: a misspelt argument is an error the
/// caller sees, never a filter that silently matches everything.
pub(super) fn args<T: DeserializeOwned>(request: Map<String, Value>) -> Result<T, Failure> {
    serde_json::from_value(Value::Object(request))
        .map_err(|error| Failure::new(format!("bad arguments: {error}"), "bad-request"))
}

/// Refuses any argument to a verb that takes none.
fn no_args(request: &Map<String, Value>) -> Result<(), Failure> {
    match request.keys().next() {
        None => Ok(()),
        Some(key) => Err(Failure::new(
            format!("bad arguments: this verb takes none, and was given {key:?}"),
            "bad-request",
        )),
    }
}

/// The argument every verb that acts on one widget takes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Target {
    pub(super) h: u64,
}

#[cfg(test)]
pub(super) fn verb_names() -> Vec<&'static str> {
    VERBS.iter().map(|(verb, _)| *verb).collect()
}
