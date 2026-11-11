//! Waiting: for a widget to appear or go, and for the app to go quiet.
//!
//! Both poll on the main loop, between the frames the app draws, so a wait never blocks the app it
//! is watching.

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{
    protocol::{Answer, Failure, args},
    query::{self, Filter},
    tree,
};

/// How often a wait looks again: about three frames.
const POLL: Duration = Duration::from_millis(50);

/// Every change signal the core has sent, counted. A settle that saw this move has seen the core
/// still working, however still the widgets looked.
static SURFACES: AtomicU64 = AtomicU64::new(0);

/// Called for each change signal the core sends.
pub(crate) fn surface_changed() {
    SURFACES.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn ping() -> Value {
    json!({ "pid": std::process::id(), "surfaces": SURFACES.load(Ordering::Relaxed) })
}

/// What `wait` takes: a filter, and `absent` and `timeoutMs` beside it. Parsed by hand rather than
/// with serde's `flatten`, which would let a misspelt filter field through unnoticed.
pub(super) struct WaitArgs {
    filter: Filter,
    absent: bool,
    timeout_ms: u64,
}

impl WaitArgs {
    pub(super) fn parse(mut request: Map<String, Value>) -> Result<Self, Failure> {
        let absent = match request.remove("absent") {
            None => false,
            Some(Value::Bool(absent)) => absent,
            Some(_) => return Err(Failure::new("absent is true or false", "bad-request")),
        };
        let timeout_ms = match request.remove("timeoutMs") {
            None => ten_seconds(),
            Some(value) => value
                .as_u64()
                .ok_or_else(|| Failure::new("timeoutMs is a whole number", "bad-request"))?,
        };
        Ok(Self {
            filter: args(request)?,
            absent,
            timeout_ms,
        })
    }
}

fn ten_seconds() -> u64 {
    10_000
}

pub(super) async fn wait(args: WaitArgs) -> Answer {
    let started = Instant::now();
    let deadline = started + Duration::from_millis(args.timeout_ms);
    loop {
        let found = query::find(&args.filter)?;
        if found.is_empty() == args.absent {
            return Ok(json!({
                "elements": found.iter().map(tree::describe).collect::<Vec<_>>(),
                "ms": started.elapsed().as_millis(),
            }));
        }
        if Instant::now() >= deadline {
            let failure = if args.absent {
                Failure::new(
                    format!("still there after {} ms", args.timeout_ms),
                    "timeout",
                )
                .with(
                    "elements",
                    json!(found.iter().map(tree::summary).collect::<Vec<_>>()),
                )
            } else {
                Failure::new(
                    format!("nothing matched within {} ms", args.timeout_ms),
                    "timeout",
                )
                .with("near", query::near(&args.filter))
            };
            return Err(failure);
        }
        gtk::glib::timeout_future(POLL).await;
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SettleArgs {
    #[serde(default = "quiet")]
    quiet_ms: u64,
    #[serde(default = "ten_seconds")]
    cap_ms: u64,
}

fn quiet() -> u64 {
    300
}

/// Waits until neither the widgets on screen nor the core have changed for `quietMs`. Answers
/// `settled: false` at the cap rather than failing: an app with a spinner never settles, and the
/// caller decides whether that matters.
pub(super) async fn settle(args: SettleArgs) -> Answer {
    let started = Instant::now();
    let quiet = Duration::from_millis(args.quiet_ms);
    let cap = Duration::from_millis(args.cap_ms);
    let mut last = fingerprint();
    let mut still_since = Instant::now();
    loop {
        gtk::glib::timeout_future(POLL).await;
        let now = fingerprint();
        if now != last {
            last = now;
            still_since = Instant::now();
        } else if still_since.elapsed() >= quiet {
            return Ok(json!({ "ms": started.elapsed().as_millis(), "settled": true }));
        }
        if started.elapsed() >= cap {
            return Ok(json!({ "ms": started.elapsed().as_millis(), "settled": false }));
        }
    }
}

/// A hash of what is on screen and how far the core has got: every showing widget's line, and the
/// count of change signals.
fn fingerprint() -> u64 {
    let mut hasher = DefaultHasher::new();
    SURFACES.load(Ordering::Relaxed).hash(&mut hasher);
    for window in tree::toplevels() {
        hash_tree(&window, &mut hasher);
    }
    hasher.finish()
}

fn hash_tree(widget: &gtk::Widget, hasher: &mut DefaultHasher) {
    use gtk::prelude::WidgetExt;
    if !widget.is_mapped() {
        return;
    }
    tree::summary(widget).hash(hasher);
    for child in tree::children(widget) {
        hash_tree(&child, hasher);
    }
}
