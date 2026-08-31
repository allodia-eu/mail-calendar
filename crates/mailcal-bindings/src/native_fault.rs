//! **Native faults**: what the log says when the process dies underneath Rust's own machinery.
//!
//! A SIGSEGV inside the cdylib, or an `abort()` under it, is neither a Rust panic nor a host
//! exception. The panic hook ([`crate::crash`]) never runs, the host's uncaught-handler never
//! runs, and the log simply stops; mid-line, with nothing wrong on the last one. That is the
//! silence this writes into: one banner naming the signal and the address that faulted.
//!
//! **The previous handler is restored and re-raised, never `SIG_DFL`.** Something is already
//! installed for these signals on every platform that reaches here; Rust's own runtime reports a
//! stack overflow from a SIGSEGV handler, and a platform crash reporter sits under that, so
//! replacing the chain with the default disposition would trade a full crash report for one line
//! in a file. Chaining leaves whatever the platform produced before this existed unchanged.
//! (Apple's client can use `SIG_DFL` because Darwin reports through Mach exceptions rather than
//! the signal disposition. It is the one platform that may.)
//!
//! Everything below the install runs with the process already dying, where only async-signal-safe
//! calls are permitted: `open`, `write`, `close`, `strlen`, `sigaction`, `raise`. No allocation, no
//! formatting, no locks, and no first touch of a lazy global; every byte is prepared at install.

use std::sync::atomic::{AtomicBool, Ordering};

/// Arms the native-fault handler, writing its records to `log_path`.
///
/// The host passes the **path**, not an open descriptor, and the handler opens it late, for the
/// rotation reason given at the write. Call it once the log file exists: a record written before
/// the sink has a file goes nowhere, the same ordering rule the panic hook and every host handler
/// follow (`docs/logging.md` → "A handler runs only if the sink is already open").
///
/// Three implementations sit behind this one name. POSIX arms `sigaction` handlers below; Windows
/// arms a vectored exception handler (`crate::native_fault_windows`), because a native fault there
/// is not a signal at all; Apple is a **no-op**, because its client installs its own handler in
/// Swift (`CrashLog.swift`) and can symbolize frames in-process, which neither of the others can.
/// Exported unconditionally so the FFI surface is one shape on every platform.
#[uniffi::export]
pub fn watch_for_native_faults(log_path: String) {
    #[cfg(all(unix, not(target_vendor = "apple")))]
    armed::install(&log_path);
    #[cfg(windows)]
    crate::native_fault_windows::install(&log_path);
    #[cfg(not(any(all(unix, not(target_vendor = "apple")), windows)))]
    let _ = log_path;
}

/// Whether a fault handler is writing to the log file **right now**.
///
/// Read by the log bridge (`crate::logging`) before it forwards a record to the host sink. The
/// handler writes with raw `write(2)` on a descriptor of its own while the ordinary path is still
/// writing through the host's; two writers, no shared lock, and no way to take one from a signal
/// handler. Standing down keeps another thread's line out of the middle of a fault record.
///
/// It lasts for the record and no longer. A caught signal does not always end the process: one
/// sent from outside arrives where nothing faulted, so the displaced handler returns and execution
/// resumes. Left raised, this would silence every remaining line of the session: the log
/// "Share log" hands over: for a process that went on running.
pub(crate) fn is_writing_fault_record() -> bool {
    WRITING.load(Ordering::Relaxed)
}

/// Raised for the length of one record, and lowered again after the last byte of it.
static WRITING: AtomicBool = AtomicBool::new(false);

/// Set by the first fault to write a record and never cleared, so a process that survives one
/// signal and is then hit by another does not append a second record to the first.
///
/// Only [`armed`] reads it, and Apple compiles that module under `cfg(test)` alone: so on a
/// non-test Apple build this is genuinely unused, and `dead_code` is denied. [`WRITING`] needs no
/// such exemption: the log bridge reads it on every platform.
#[cfg_attr(
    target_vendor = "apple",
    allow(
        dead_code,
        reason = "Apple arms no handler here; its client owns one in Swift"
    )
)]
static CLAIMED: AtomicBool = AtomicBool::new(false);

/// Raises and lowers the stand-down that [`is_writing_fault_record`] reports.
///
/// The POSIX handler is a child of this module and holds [`WRITING`] itself; the Windows one is a
/// sibling and reaches it through here.
#[cfg(windows)]
pub(crate) fn writing_fault_record(writing: bool) {
    WRITING.store(writing, Ordering::Relaxed);
}

/// Claims the log for a fault record, returning whether this caller is the first to do so.
///
/// Called before a byte is written, by both handlers. Two threads faulting at once means only the
/// first writes; the second falls straight through to whatever it was going to do next rather than
/// interleaving with it.
#[cfg_attr(
    target_vendor = "apple",
    allow(
        dead_code,
        reason = "Apple arms no handler here; its client owns one in Swift"
    )
)]
pub(crate) fn claim_log() -> bool {
    !CLAIMED.swap(true, Ordering::Relaxed)
}

// Compiled on Apple under `cfg(test)` even though its client never arms it, so the install, the
// chain and the write are exercised by the one gate that runs on this repo's dev machines. Without
// it every assertion below reports `0 tests ... ok` on macOS, which reads exactly like a pass;
// the failure mode ../../AGENTS.md names for any `cfg`-split branch.
#[cfg(all(unix, any(not(target_vendor = "apple"), test)))]
mod armed {
    use std::{
        ffi::{CString, c_char, c_int, c_void},
        ptr,
        sync::{
            Once,
            atomic::{AtomicPtr, Ordering},
        },
    };

    use super::{WRITING, claim_log};
    use crate::native_fault_record::{SUFFIX, hex_into, signal_prefix};

    /// The faults worth catching, with the name each writes.
    ///
    /// Deliberately **not** SIGPIPE (Rust ignores it by design) and not one of the polite
    /// termination signals; catching SIGINT or SIGTERM would file an ordinary quit as a crash,
    /// which is the exact confusion this feature exists to remove, in reverse.
    const WATCHED: [(c_int, &str); 6] = [
        (libc::SIGABRT, "SIGABRT"),
        (libc::SIGBUS, "SIGBUS"),
        (libc::SIGFPE, "SIGFPE"),
        (libc::SIGILL, "SIGILL"),
        (libc::SIGSEGV, "SIGSEGV"),
        (libc::SIGTRAP, "SIGTRAP"),
    ];

    /// Room for one prepared record per signal number, indexed by the number itself so the handler
    /// reaches its own without searching. 32 covers every signal in [`WATCHED`] on both platforms.
    const SLOTS: usize = 32;

    /// How many frames the Linux record carries. Deep enough to cross the fault into the code that
    /// caused it, shallow enough not to bury the rest of the log under one record.
    #[cfg(target_os = "linux")]
    const FRAMES: usize = 64;

    /// The log path as a C string, prepared at install because the handler cannot allocate one.
    static LOG_PATH: AtomicPtr<c_char> = AtomicPtr::new(ptr::null_mut());

    /// One pre-rendered opening per watched signal, indexed by signal number. Same reason.
    static PREFIXES: [AtomicPtr<c_char>; SLOTS] =
        [const { AtomicPtr::new(ptr::null_mut()) }; SLOTS];

    /// The handler each slot displaced, so the fault can be handed back to it.
    static PREVIOUS: [AtomicPtr<libc::sigaction>; SLOTS] =
        [const { AtomicPtr::new(ptr::null_mut()) }; SLOTS];

    // `backtrace_symbols_fd` writes frames without allocating, which is why it, and not its
    // `backtrace_symbols` sibling, is the only one usable here. A plain comment, not a doc one:
    // rustdoc generates nothing for an extern block and `unused_doc_comments` is denied.
    //
    // Declared rather than taken from `libc` so the availability question is answered in one
    // place. Bionic gained all three at API 33 (`__INTRODUCED_IN(33)` in the NDK's `execinfo.h`)
    // and this client's `minSdk` is 31, so Android gets the banner alone. Widening this `cfg` to
    // Android does not build: the NDK's stub `libc.so` carries the symbols from API 33 up and the
    // cdylib links against a lower one, so the link fails on `undefined symbol: backtrace`.
    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
        fn backtrace_symbols_fd(buffer: *const *mut c_void, size: c_int, fd: c_int);
    }

    /// Prepares every byte the handler will need, then arms it. Once per process: a second call
    /// would record *this* handler as the one to chain to and loop the fault back into itself.
    pub(super) fn install(log_path: &str) {
        static ARMED: Once = Once::new();
        ARMED.call_once(|| {
            let Ok(path) = CString::new(log_path) else {
                return; // A path with an interior NUL cannot be opened, nothing to arm against.
            };
            // Leaked on purpose, all of it: the handler may read these at any point up to the last
            // instruction the process executes, so nothing here may ever be freed.
            LOG_PATH.store(path.into_raw(), Ordering::Relaxed);
            for (number, name) in WATCHED {
                let slot = number as usize;
                if slot >= SLOTS {
                    continue;
                }
                let Ok(opening) = CString::new(signal_prefix(name)) else {
                    continue;
                };
                PREFIXES[slot].store(opening.into_raw(), Ordering::Relaxed);
                arm(number, slot);
            }
        });
    }

    /// Installs the handler for one signal, remembering what it displaced.
    fn arm(number: c_int, slot: usize) {
        // SAFETY: `action` is fully initialised before it is passed, and `previous` is a leaked
        // allocation read only by the handler, which cannot outlive the process.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handle as *const () as usize;
            libc::sigemptyset(&raw mut action.sa_mask);
            // SA_SIGINFO carries the faulting address; SA_ONSTACK runs on the alternate stack the
            // Rust runtime already installs, without which a stack-overflow SIGSEGV has no room to
            // run a handler at all.
            action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
            let previous = Box::into_raw(Box::new(std::mem::zeroed::<libc::sigaction>()));
            if libc::sigaction(number, &raw const action, previous) == 0 {
                PREVIOUS[slot].store(previous, Ordering::Relaxed);
            } else {
                drop(Box::from_raw(previous));
            }
        }
    }

    /// Writes the record, then hands the fault back to the handler this one displaced.
    ///
    /// Restoring and re-raising is what keeps the platform's own crash reporting in play. The
    /// signal stays blocked for as long as this function runs, so the re-raised one is delivered on
    /// return; to the restored handler, which sees the fault as it would have without us.
    extern "C" fn handle(number: c_int, info: *mut libc::siginfo_t, _context: *mut c_void) {
        // First, before a byte is written: it is what stops another thread's line landing
        // mid-record. Two threads faulting at once means only the first writes; the second falls
        // straight through to the re-raise rather than interleaving with it.
        let first = claim_log();
        let slot = (number as usize).min(SLOTS - 1);
        if first {
            WRITING.store(true, Ordering::Relaxed);
            // SAFETY: every pointer read here was prepared by `install` and leaked; `info` is the
            // kernel's own, valid for the duration of this call.
            unsafe {
                let address = if info.is_null() {
                    0
                } else {
                    (*info).si_addr() as usize
                };
                write_record(PREFIXES[slot].load(Ordering::Relaxed), address);
            }
            // The record is closed, so the ordinary log has nothing left to interleave with.
            // Lowered before the re-raise rather than after it, because for a real fault there is
            // no "after": the displaced handler ends the process.
            WRITING.store(false, Ordering::Relaxed);
        }
        // SAFETY: `previous` is the leaked `sigaction` this handler displaced at install.
        unsafe {
            let previous = PREVIOUS[slot].load(Ordering::Relaxed);
            if previous.is_null() {
                libc::signal(number, libc::SIG_DFL);
            } else {
                libc::sigaction(number, previous, ptr::null_mut());
            }
            libc::raise(number);
        }
    }

    /// The write itself: opening, address, tail, and, where the platform can manage it without
    /// allocating: the frames.
    ///
    /// The log is opened here rather than held open from install, which the obvious design would
    /// do. `open(2)` is async-signal-safe, and a descriptor kept across the life of the process
    /// would pin the inode: after one rotation it would still be attached to `mailcal.log.1`, so
    /// the record would land in a backup: a file "Share log" does not hand over.
    unsafe fn write_record(opening: *const c_char, address: usize) {
        let path = LOG_PATH.load(Ordering::Relaxed);
        if path.is_null() {
            return;
        }
        // SAFETY: every pointer below is either leaked at install or a stack buffer owned here.
        unsafe {
            let fd = libc::open(path, libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT, 0o644);
            if fd < 0 {
                return;
            }
            if !opening.is_null() {
                write_all(fd, opening.cast(), libc::strlen(opening));
            }
            let mut hex = [0u8; 18];
            let rendered = hex_into(address, &mut hex);
            write_all(fd, rendered.as_ptr().cast(), rendered.len());
            write_all(fd, SUFFIX.as_ptr().cast(), SUFFIX.len());
            #[cfg(target_os = "linux")]
            {
                let mut frames = [ptr::null_mut::<c_void>(); FRAMES];
                // `try_into` rather than a cast: the pedantic lint set denies `usize as c_int`,
                // and the fallback is unreachable for a constant this size.
                let depth = backtrace(frames.as_mut_ptr(), FRAMES.try_into().unwrap_or(c_int::MAX));
                backtrace_symbols_fd(frames.as_ptr(), depth, fd);
            }
            libc::close(fd);
        }
    }

    /// `write(2)` until the buffer is out or the descriptor refuses it. A short write is ordinary
    /// on a signal-interrupted descriptor, and a record cut in half is the thing this module
    /// exists to stop producing.
    unsafe fn write_all(fd: c_int, mut buf: *const c_void, mut len: usize) {
        // SAFETY: `buf` is valid for `len` bytes and the loop only ever advances within it.
        unsafe {
            while len > 0 {
                let written = libc::write(fd, buf, len);
                if written <= 0 {
                    return;
                }
                let written = written as usize;
                buf = buf.cast::<u8>().add(written).cast();
                len -= written;
            }
        }
    }
}

#[cfg(all(unix, test))]
#[path = "native_fault_faulting.rs"]
mod faulting;
