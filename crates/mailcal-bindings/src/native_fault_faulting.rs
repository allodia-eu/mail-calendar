//! The handler driven for real: a child process is armed, made to fault, and its log read back.
//!
//! Nothing short of an actual fault proves any of this. The install could silently fail, the
//! record could land in the wrong file, the chain could be dropped, and every unit test in
//! [`super`] would still pass, because each one only asks what a string looks like.
//!
//! # The child is spawned, not forked
//!
//! `fork(2)` from a process with other threads running gives a child that may call only
//! async-signal-safe functions until it execs, and `armed::install` cannot honour that: it
//! allocates, by construction, a `CString` per signal name and a `Box<sigaction>` per handler it
//! displaces. libtest runs these tests on threads beside a hundred and fifty others, so the fork
//! landed in a process where another thread could be inside the allocator.
//!
//! What that produced in CI was not a hang but a `Box::into_raw` the chain could not use:
//! `PREVIOUS[slot]` held a non-null pointer that `sigaction` refused with `EFAULT`, leaving the
//! armed handler in place, so the `raise` at the end of it re-entered the handler until the
//! alternate stack ran out. Both tests then died of SIGSEGV whatever signal they had raised, which
//! is why the SIGTRAP one reported status 139 rather than 133.
//!
//! Spawning the test binary again removes the hazard rather than narrowing the window: the child is
//! a fresh process that never forked, so every allocation in it is an ordinary one.

use std::{
    ffi::{CString, c_char},
    fs, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use super::armed;

/// What the displaced handler writes, and the status it exits with.
///
/// Exiting rather than returning is not a detail: returning from a handler for a synchronous
/// fault re-executes the faulting instruction, so a chain that merely *ran* would spin forever.
/// A distinctive exit status is the only evidence that reaches the parent.
const CHAINED: &str = "[the displaced handler ran]\n";
const CHAINED_STATUS: i32 = 42;

/// What the survivor writes once it is running normally again, and the status it exits with.
const SURVIVED: &str = "[the ordinary log is live again]\n";
const SILENCED: &str = "[the ordinary log is still standing down]\n";
const SURVIVED_STATUS: i32 = 43;

/// The child's log path, stored before the handler is armed so it only ever reads it.
static LOG_PATH: AtomicPtr<c_char> = AtomicPtr::new(ptr::null_mut());

/// The handler `install` will displace. Async-signal-safe, like the one replacing it.
extern "C" fn previous(_number: libc::c_int) {
    let path = LOG_PATH.load(Ordering::Relaxed);
    // SAFETY: `path` was leaked before the handler was armed; the rest is
    // `open`/`write`/`close`/`_exit`,
    // every one of them on the async-signal-safe list.
    unsafe {
        if !path.is_null() {
            let fd = libc::open(path, libc::O_WRONLY | libc::O_APPEND);
            if fd >= 0 {
                libc::write(fd, CHAINED.as_ptr().cast(), CHAINED.len());
                libc::close(fd);
            }
        }
        libc::_exit(CHAINED_STATUS);
    }
}

/// The environment variable that turns a spawned run of this binary into the child half.
///
/// Its absence is what keeps `cargo test -- --ignored` from faulting a harness: the child tests
/// return immediately unless the parent asked for them by name and handed them a log.
const CHILD_LOG: &str = "MAILCAL_FAULT_CHILD_LOG";

/// The log this run is the child of, or `None` when it is an ordinary test run.
fn child_log() -> Option<String> {
    std::env::var(CHILD_LOG).ok()
}

/// A scratch directory and a seeded log, named for the test so two running at once cannot collide.
fn scratch(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mailcal-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch dir");
    let log = dir.join("mailcal.log");
    fs::write(&log, "an ordinary line\n").expect("seed the log");
    (dir, log)
}

/// Runs one `#[ignore]`d child test in a **new process** and reports how it left.
///
/// `--test-threads=1` is not tidiness: the child arms process-wide signal handlers and then faults,
/// so a second test running beside it would be sharing the disposition of the signal that is about
/// to end the process.
fn run_child(test: &str, log: &std::path::Path) -> std::process::ExitStatus {
    std::process::Command::new(std::env::current_exe().expect("the test binary's own path"))
        .args([
            test,
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_LOG, log)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("spawn the child")
}

/// How the child left, for a message that can tell an exit code from a signal.
fn how_it_left(status: &std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt as _;
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("exit {code}"),
        (None, Some(signal)) => format!("killed by signal {signal}"),
        _ => format!("{status:?}"),
    }
}

#[test]
#[ignore = "the child half of a_real_fault_...; that test spawns it by name"]
fn the_faulting_child() {
    let Some(log) = child_log() else { return };
    let path = CString::new(log.as_bytes()).expect("path");
    // Before the handler is armed, so `previous` only ever reads what is already there.
    LOG_PATH.store(path.into_raw(), Ordering::Relaxed);

    // SAFETY: an ordinary single-purpose process. It arms two handlers, faults, and leaves through
    // whichever of them runs; nothing after the write is reachable.
    unsafe {
        libc::signal(libc::SIGSEGV, previous as *const () as usize);
        armed::install(&log);
        // Volatile so the optimizer cannot decide a null write is unreachable and drop it.
        std::ptr::write_volatile(std::ptr::null_mut::<u8>(), 1);
        libc::_exit(0); // Unreachable; a fault that did not happen must not read as a pass.
    }
}

#[test]
fn a_real_fault_writes_its_record_and_still_reaches_the_handler_it_displaced() {
    let (dir, log) = scratch("fault");
    let status = run_child("native_fault::faulting::the_faulting_child", &log);
    let written = fs::read_to_string(&log).expect("read the log back");
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(
        status.code(),
        Some(CHAINED_STATUS),
        "the fault never reached the displaced handler ({}); log:\n{written}",
        how_it_left(&status)
    );
    assert!(
        written.contains("unhandled native fault SIGSEGV at 0x0"),
        "no record, or the wrong address, in:\n{written}"
    );
    assert!(
        written.contains(CHAINED),
        "the chain was dropped; log:\n{written}"
    );
    assert!(
        written.starts_with("an ordinary line\n"),
        "the record replaced the log instead of appending to it:\n{written}"
    );
    #[cfg(target_os = "linux")]
    frames_landed_inside_the_record(&written);
    #[cfg(target_os = "android")]
    the_record_is_the_banner_alone(&written);
}

/// A handler that returns, leaving the process running; what an externally sent signal gets.
extern "C" fn recovers(_number: libc::c_int) {}

#[test]
#[ignore = "the child half of a_fault_the_process_survives_...; that test spawns it by name"]
fn the_surviving_child() {
    let Some(log) = child_log() else { return };

    // SAFETY: as above. This one is expected to come back from its own signal, so it runs on past
    // the raise and leaves deliberately.
    unsafe {
        // A signal sent from outside is delivered where nothing faulted, so the handler this one
        // displaces returns and execution carries on past the raise.
        libc::signal(libc::SIGTRAP, recovers as *const () as usize);
        armed::install(&log);
        libc::raise(libc::SIGTRAP);
        // Back in ordinary code, where the log bridge decides whether to forward a line.
        let verdict = if super::is_writing_fault_record() {
            SILENCED
        } else {
            SURVIVED
        };
        let path = CString::new(log.as_bytes()).expect("path");
        let fd = libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_APPEND);
        if fd >= 0 {
            libc::write(fd, verdict.as_ptr().cast(), verdict.len());
            libc::close(fd);
        }
        libc::_exit(SURVIVED_STATUS);
    }
}

#[test]
fn a_fault_the_process_survives_leaves_the_ordinary_log_working() {
    let (dir, log) = scratch("survived");
    let status = run_child("native_fault::faulting::the_surviving_child", &log);
    let written = fs::read_to_string(&log).expect("read the log back");
    let _ = fs::remove_dir_all(&dir);

    assert_eq!(
        status.code(),
        Some(SURVIVED_STATUS),
        "the child did not survive its own signal ({}); log:\n{written}",
        how_it_left(&status)
    );
    assert!(
        written.contains("unhandled native fault SIGTRAP at"),
        "no record for a signal the process survived:\n{written}"
    );
    assert!(
        written.contains(SURVIVED),
        "the stand-down outlived the record, so every remaining line of the session would \
         have been dropped from the log the user hands over:\n{written}"
    );
}

/// What the fault wrote after the banner, up to the displaced handler's line.
///
/// Whatever a platform puts in a record goes here: after the opening it belongs to, before the
/// chain that follows it. Both platforms with an arm of their own assert on this slice.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn record_after_the_banner<'a>(written: &'a str, chained: &str) -> &'a str {
    let after_banner = written
        .split_once(crate::native_fault_record::SUFFIX)
        .expect("the banner is present")
        .1;
    after_banner
        .split_once(chained)
        .expect("the chained line follows the record")
        .0
}

/// A frame is `path(+0xoffset)[0xaddress]`. The bracketed address is the part
/// `backtrace_symbols_fd` writes whether or not a symbol name resolved, so it is what counting
/// frames keys on.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn frames_in(record: &str) -> usize {
    record.lines().filter(|line| line.contains("[0x")).count()
}

/// Linux writes frames after the banner, and only a real fault can show that it did.
///
/// `backtrace_symbols_fd` is the one call here that is reached on no other platform, so without
/// this the whole Linux branch is compiled and never observed.
#[cfg(target_os = "linux")]
fn frames_landed_inside_the_record(written: &str) {
    let record = record_after_the_banner(written, CHAINED);
    let frames = frames_in(record);

    assert!(
        frames >= 3,
        "the fault carried {frames} frames, so the Linux backtrace path wrote nothing usable \
         (the record between the banner and the chain was):\n{record}"
    );
}

/// Android's record is the banner and nothing else, and that is a rule rather than a shortfall.
///
/// Bionic gained `backtrace(3)` and `backtrace_symbols_fd(3)` at API 33 and this client's
/// `minSdk` is **31**. Widening the Linux `cfg` in [`super`] never reaches this assertion; it
/// fails to link first: so what this pins is the shape of the record support actually reads on
/// Android: the banner, and frames arriving by no other route either.
#[cfg(target_os = "android")]
fn the_record_is_the_banner_alone(written: &str) {
    let record = record_after_the_banner(written, CHAINED);
    let frames = frames_in(record);

    assert!(
        frames == 0,
        "the fault carried {frames} frames on Android, where bionic gains `backtrace` only at \
         API 33 and this client's minSdk is 31:\n{record}"
    );
}
