//! What the log holds after a panic, asserted against a capturing host [`Logger`].
//!
//! `crash.rs`'s unit test pins the *wording*, nothing there proves a real panic ever reaches a
//! sink. This drives the actual hook: build an app through the public FFI so the hook is armed the
//! way a client arms it, panic on a worker thread, and read what a support engineer would.
//!
//! # Why this is an integration test and not a unit one
//!
//! `install_logger` swaps a **process-global** sink and `set_hook` arms a **process-global** hook,
//! and the crate's unit tests build dozens of apps in parallel in one process: so a capture
//! installed in `src/` loses its records the moment another test constructs an app.
//! `credential_logging.rs` documents the same reason at length. Cargo gives each integration file
//! its own process; this one builds the only app in it.
//!
//! The panic is raised on a **spawned** thread on purpose: that is the case with no other witness
//! (a task dies and the app carries on), and it keeps the panic off the thread running the test.

use std::sync::{Arc, Mutex, mpsc};

use mailcal_bindings::{LogLevel, Logger, MailcalApp, Observer, Surface};

/// A host logger that keeps every record the core emits.
struct Capture(Arc<Mutex<Vec<String>>>);

impl Logger for Capture {
    fn log(&self, _level: LogLevel, target: String, message: String) {
        self.0
            .lock()
            .expect("capture mutex poisoned")
            .push(format!("{target} {message}"));
    }
}

/// The observer the FFI requires; this test asserts on logs, not snapshots.
struct SilentObserver(mpsc::Sender<()>);

impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {
        let _ = self.0.send(());
    }
}

/// A payload that cannot occur by accident, so finding it is a substring search that cannot
/// collide with an unrelated line.
const PAYLOAD: &str = "quorrix went sideways";

/// Panics on a named thread and returns once the hook has run.
fn panic_on_a_named_thread(name: &str) {
    let raised = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(|| panic!("{PAYLOAD}"))
        .expect("the thread spawns")
        .join();
    assert!(raised.is_err(), "the spawned thread is meant to panic");
}

/// The stack's frame lines, as `Backtrace` prints them: `<n>: <symbol>`, one per frame.
///
/// The presence of a stack is asserted on this shape rather than on a symbol from std's own
/// capture machinery, because how much of that machinery std trims off the top differs between
/// platforms, and the tidier stack is the one such an assertion fails on.
fn numbered_frames(record: &str) -> Vec<&str> {
    record
        .lines()
        .filter(|line| {
            let frame = line.trim_start();
            let digits = frame.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && frame[digits..].starts_with(": ")
        })
        .collect()
}

/// Every crash record the capture holds, in order.
fn crash_records(lines: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    lines
        .lock()
        .expect("capture mutex poisoned")
        .iter()
        .filter(|line| line.contains("unhandled panic"))
        .cloned()
        .collect()
}

#[test]
fn a_panic_on_a_worker_thread_reaches_the_log_with_its_place_and_its_stack() {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let (tx, _rx) = mpsc::channel();
    // The demo app is the cheapest constructor (in-memory, no network, no store) and it installs
    // the logger and arms the hook through exactly the path a client uses.
    let _app = MailcalApp::new_demo(
        Box::new(SilentObserver(tx)),
        Box::new(Capture(Arc::clone(&lines))),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );

    let thread_name = "quorrix-worker";
    panic_on_a_named_thread(thread_name);

    let records = crash_records(&lines);
    // One record, not two. A header and a stack written separately can be split apart by a
    // concurrent writer, and half a stack under someone else's line is worse than none.
    assert_eq!(records.len(), 1, "the panic wrote exactly one record");
    let record = &records[0];

    // The four things a support log has to answer: that it was a crash, whose thread died, where,
    // and what the code was doing to get there.
    assert!(record.contains(thread_name), "{record}");
    assert!(record.contains(PAYLOAD), "{record}");
    assert!(
        record.contains("panic_logging.rs:"),
        "the record names the file and line the panic came from: {record}"
    );
    let frames = numbered_frames(record);
    assert!(
        frames.len() >= 3,
        "the record carries a stack, not just a headline: {record}"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("panic_on_a_named_thread")),
        "the stack reaches the code that panicked: {record}"
    );

    // A watcher that dies on a malformed response panics again on every reconnect, and identical
    // stacks would fill the sink's rotating cap and evict what explains the first one. Past the
    // cap the headline still reports each repeat: only the frames stop.
    for repeat in 0..4 {
        panic_on_a_named_thread(&format!("quorrix-repeat-{repeat}"));
    }
    let records = crash_records(&lines);
    assert_eq!(records.len(), 5, "every panic is still reported");
    let last = records.last().expect("five records");
    assert!(
        last.contains(PAYLOAD),
        "the headline survives the cap: {last}"
    );
    assert!(
        numbered_frames(last).is_empty(),
        "a repeat past the cap carries no stack: {last}"
    );
}
