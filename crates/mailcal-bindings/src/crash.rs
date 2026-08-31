//! The **panic hook**: what the diagnostic log says when Rust code dies.
//!
//! A panic is the core's crash. Without a hook it is invisible on every client: one on a runtime
//! worker thread kills that task and writes nothing, and one under a host-initiated call is caught
//! at the FFI boundary and handed to the host as a message alone: so the file and line it came
//! from exist nowhere at all. This hook is the only place that information is ever recorded.
//!
//! What crosses the FFI, and why that is not enough (UniFFI 0.31, `rust_call_with_out_status`):
//! a caught panic's payload is downcast to a string and lowered into the host's error, which
//! becomes a Kotlin `InternalException`, a Swift `MailcalException`, a C# `UniffiException`. Its
//! *stack* is the host's, starting at the generated binding: the two runtimes unwind on separate
//! machinery, so there is no such thing as one trace spanning both. A payload that is not a string
//! arrives as `Unknown panic!` and even the message is gone.
//!
//! So the two halves are joined in the **file**, not in one stack: this record and the host's own
//! unhandled-exception line land in the same log, moments apart, carrying the same payload text.
//! Support greps `unhandled` and reads both.

use std::{
    backtrace::Backtrace,
    panic::{self, PanicHookInfo},
    sync::{
        Once,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

/// How many panics get a stack before the log keeps headlines only.
///
/// A stack is ~40 lines, and a panic is rarely a single event: a watcher that dies on a malformed
/// server response panics again on every reconnect. Unbounded, those identical stacks fill the
/// sink's 1 MB rotating cap and evict the very history that explains the first one: the same
/// eviction `docs/logging.md` argues against for `debug`-level dependency noise. Three is enough
/// to see the stack and to see it repeat; after that the headline alone still says it is ongoing.
const STACKS_KEPT: usize = 3;

/// Stacks written so far. Saturates rather than wraps: the count is only ever compared to
/// [`STACKS_KEPT`].
static STACKS_WRITTEN: AtomicUsize = AtomicUsize::new(0);

/// Arms the process-wide panic hook, once per process.
///
/// Called from [`crate::logging::install_logger`], which every boot path runs before any work, so
/// the hook is in place for the whole life of the process. The guard is required, not defensive:
/// `install_logger` runs on every constructor, and each `set_hook` would otherwise stack another
/// copy of the previous chain and write the record once per constructor ever built.
pub(crate) fn install_panic_hook() {
    static ARMED: Once = Once::new();
    ARMED.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            // The previous hook first. It is the default one (stderr, and Logcat/Console with it),
            // and it cannot fail; ours crosses the FFI into a host sink, and a sink that panics
            // there kills the process outright: a panic raised while panicking aborts before any
            // hook runs again. Both destinations get the record; the one that cannot be taken away
            // goes first.
            previous(info);
            log::error!("{}", record(info));
        }));
    });
}

/// The single record a panic writes.
///
/// **One record, not two.** Each sink serializes a whole message under its own lock, so a header
/// line and a separate stack could be split apart by a concurrent writer, and half a stack under
/// someone else's line is worse than none. The multi-line shape matches what Windows already
/// writes for a .NET exception.
fn record(info: &PanicHookInfo<'_>) -> String {
    let written = STACKS_WRITTEN.fetch_add(1, Ordering::Relaxed);
    // `force_capture` ignores `RUST_BACKTRACE`, so a shipped build gets a stack too, and it is
    // only paid for when one is going to be written.
    let stack = if written < STACKS_KEPT {
        Backtrace::force_capture().to_string()
    } else {
        "(this run has already recorded a stack; a repeat says only that it is still happening)"
            .to_owned()
    };
    let thread = thread::current();
    compose(
        info.payload_as_str().unwrap_or("no message"),
        &info.location().map_or_else(
            || "an unrecorded place".to_owned(),
            std::string::ToString::to_string,
        ),
        thread.name().unwrap_or("an unnamed thread"),
        &stack,
    )
}

/// Composes the record's text. Separated from [`record`] so the wording is pinned by a unit test
/// without raising a panic to produce one.
///
/// `unhandled` is the word every platform's crash line carries; one string support greps across
/// four clients, and this line does **not** claim the process is ending: a panic under a
/// host-initiated call is converted to an error the host may well survive, and one on a worker
/// thread ends that work alone.
///
/// The stack's first frames are this hook's own, because it is capturing from inside the panic
/// runtime. `location` is the answer to "where"; the frames are the answer to "how it got there".
fn compose(payload: &str, location: &str, thread: &str, stack: &str) -> String {
    format!("unhandled panic on {thread}, at {location}: {payload}\n{stack}")
}

#[cfg(test)]
mod tests {
    use super::compose;

    #[test]
    fn the_record_leads_with_the_word_every_platform_greps() {
        let line = compose(
            "it broke",
            "somewhere.rs:42:9",
            "tokio-runtime-worker",
            "0: frame",
        );
        assert_eq!(
            line,
            "unhandled panic on tokio-runtime-worker, at somewhere.rs:42:9: it broke\n0: frame"
        );
    }
}
