// What the diagnostic log says when the Apple client is about to die (docs/logging.md → "A crash
// says so on the way out"). Without it a crash is indistinguishable in the file from a clean exit:
// the log simply stops, with nothing wrong on the last line.
//
// Three faults kill this app and only one of them is an NSException:
//
//   * an ObjC NSException (KVO, AppKit/UIKit internals)     -> NSSetUncaughtExceptionHandler
//   * a Swift trap, force-unwrap nil, index out of range,
//     fatalError, precondition                              -> SIGILL / SIGTRAP, signal only
//   * a Rust abort or a native fault in the cdylib          -> SIGABRT / SIGSEGV / SIGBUS, ditto
//
// The middle row is the one that actually bit us: the `Binding($state)!` trap in a `.sheet(item:)`
// closure is a Swift trap, and an NSException handler would have written nothing at all.
//
// MetricKit (MXCrashDiagnostic) would catch every row with none of the care below, but it
// delivers at the NEXT launch, so the log would say "the previous session ended in a crash"
// instead of having its last line explain the death. Worth adding later; not a substitute.

import Darwin
import Foundation

/// Writes the last words of a crashing process into the rotating diagnostic log.
public enum CrashLog {
    /// The signals worth catching, with the name each writes. Deliberately does **not** include
    /// SIGPIPE (Foundation and Rust both ignore it by design) or any of the polite termination
    /// signals, catching those would turn an ordinary quit into a crash report.
    static let watchedSignals: [(number: Int32, name: String)] = [
        (SIGABRT, "SIGABRT"),
        (SIGBUS, "SIGBUS"),
        (SIGFPE, "SIGFPE"),
        (SIGILL, "SIGILL"),
        (SIGSEGV, "SIGSEGV"),
        (SIGTRAP, "SIGTRAP"),
    ]

    /// Arms both handlers. Called from `AllodiaApp.init()` on every platform, earlier than any
    /// delegate callback, and the only point iOS has at all, since it has no AppKit delegate.
    public static func install() {
        // FileLog.shared is lazy and its initializer is what creates the file. Touching it here
        // means the path the signal handler writes to already exists before anything can crash.
        _ = FileLog.shared
        prepareSignalState()
        for watched in watchedSignals {
            signal(watched.number, handleFatalSignal)
        }
        // ⚠️ The exception handler is armed a run-loop turn later, and it has to be. AppKit
        // replaces the uncaught-exception handler while NSApplication sets itself up, which
        // happens AFTER this runs, so an install made here is silently discarded, and it was:
        // measured on macOS 26, an NSException raised on a background thread reached SIGABRT with
        // no line of its own, while the identical handler installed from
        // `applicationDidFinishLaunching` wrote one. `NSGetUncaughtExceptionHandler()` still
        // returns ours either way, so nothing about it looks wrong. Deferring to the first
        // main-queue turn lands after both AppKit's and UIKit's setup and needs no delegate.
        DispatchQueue.main.async {
            NSSetUncaughtExceptionHandler { exception in
                FileLog.shared.appendNow(
                    level: "ERROR",
                    target: "crash",
                    message: CrashLog.record(
                        name: exception.name.rawValue,
                        reason: exception.reason,
                        frames: exception.callStackSymbols
                    )
                )
            }
        }
        #if DEBUG
        armCrashTestTrigger()
        #endif
    }

    /// The line an uncaught NSException writes.
    ///
    /// `unhandled` is the word every platform's crash line carries, one string support greps
    /// across four clients, and the frames follow on their own lines, the same shape Windows
    /// already writes for a .NET exception.
    static func record(name: String, reason: String?, frames: [String]) -> String {
        let head = "unhandled \(name): \(reason ?? "no reason")"
        return frames.isEmpty ? head : head + "\n" + frames.joined(separator: "\n")
    }

    /// The line a fatal signal writes, ahead of its frames.
    ///
    /// It carries no timestamp, and cannot: formatting one needs allocation, which a signal
    /// handler may not do. The record is appended straight after the last timestamped line, so its
    /// position dates it to within that line, and the banners make it obvious at a glance that
    /// this block is not an ordinary entry.
    static func banner(forSignalNamed name: String) -> String {
        "\n*** unhandled signal \(name), the app stopped here ***\n"
    }
}

// MARK: - The signal path
//
// Everything below runs with the process already dying, where the only calls permitted are the
// async-signal-safe ones: `open`, `write`, `close`, `strlen`, `raise`, plus `backtrace` and
// `backtrace_symbols_fd` (the `_fd` form exists precisely because it does not allocate, its
// `backtrace_symbols` sibling mallocs and must never be used here). No Swift String, no
// DateFormatter, no DispatchQueue, and nothing that would be the first touch of a lazy global.
//
// The log is opened *inside* the handler rather than kept open from launch, which the obvious
// design would do. `open(2)` is itself async-signal-safe, and holding a descriptor across the
// life of the process would pin the inode: after one rotation it would still be attached to
// `mailcal.log.1`, so the crash record would land in a backup, a file "Share log" does not hand
// over. Opening late costs one syscall and always finds the live file.

/// Frames captured at signal time. Allocated at install, because the handler cannot allocate.
private var crashFrames = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: 128)
private let crashFrameCapacity: Int32 = 128

/// Non-zero once a fatal signal has begun writing its record.
///
/// The handler writes with raw `write(2)` on its own descriptor while `FileLog`'s serial queue is
/// still appending through its own, two writers, no shared lock, and no way to take one from a
/// signal handler. Observed on an iPhone simulator: two `DEBUG` lines landed between frames 3 and 4
/// of a SIGABRT stack. So the ordinary path stands down instead. Nothing is lost: the process is
/// already dying, and a line written after the crash record would never be read anyway.
private var crashRecordUnderway: Int32 = 0

/// Whether a fatal signal handler has claimed the log. Read by `FileLog` before each write.
func isWritingCrashRecord() -> Bool { crashRecordUnderway != 0 }

/// The log path as a C string, and one pre-formatted banner per watched signal, indexed by signal
/// number. Both are built at install time for the same reason the frame buffer is.
private var crashLogPath: UnsafeMutablePointer<CChar>?
private let crashBannerSlots = 32
private var crashBanners =
    UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>.allocate(capacity: crashBannerSlots)

/// Builds everything the handler will need, while allocation is still allowed.
private func prepareSignalState() {
    // Each of these is a Swift global, so its first access runs the lazy initializer. Touching
    // them all here means the handler only ever reads one that is already set up.
    crashRecordUnderway = 0
    crashLogPath = strdup(FileLog.shared.fileURL.path)
    crashBanners.initialize(repeating: nil, count: crashBannerSlots)
    for watched in CrashLog.watchedSignals where watched.number > 0 && Int(watched.number) < crashBannerSlots {
        crashBanners[Int(watched.number)] = strdup(CrashLog.banner(forSignalNamed: watched.name))
    }
}

/// Writes the banner and the frames, then lets the fault kill the process as it was going to.
///
/// Restoring the default disposition and re-raising is what keeps Apple's own crash reporter in
/// play: returning from a handler for a synchronous fault would re-execute the faulting
/// instruction forever.
private func handleFatalSignal(_ number: Int32) {
    // First, before a byte is written: it is what stops another thread's line landing mid-stack.
    crashRecordUnderway = 1
    if let path = crashLogPath, number > 0, Int(number) < crashBannerSlots {
        let descriptor = open(path, O_WRONLY | O_APPEND | O_CREAT, 0o644)
        if descriptor >= 0 {
            if let banner = crashBanners[Int(number)] {
                _ = write(descriptor, banner, strlen(banner))
            }
            let depth = backtrace(crashFrames, crashFrameCapacity)
            backtrace_symbols_fd(crashFrames, depth, descriptor)
            close(descriptor)
        }
    }
    signal(number, SIG_DFL)
    raise(number)
}

#if DEBUG
/// Kills this launch on purpose, a couple of seconds in, so each fault shape can be driven by hand
/// and the tail of `mailcal.log` read afterwards. There is no Apple UI-test target, so this is the
/// only way any of the above is exercised at all, and it is worth re-running whenever this file
/// moves, because both defects found while writing it were invisible to every other gate.
///
/// `MAILCAL_CRASH_TEST=objc|trap|abort|segv`, DEBUG-only, in the same style as `MAILCAL_SHOWCASE`
/// and `MAILCAL_DEV_ACCOUNT`. Never set in a shipped build, and compiled out of one.
private func armCrashTestTrigger() {
    guard let shape = ProcessInfo.processInfo.environment["MAILCAL_CRASH_TEST"] else { return }
    logAppleLifecycle("MAILCAL_CRASH_TEST=\(shape), this launch will stop on purpose")
    // On a background thread, not the main queue, and that is the point: an NSException raised on
    // macOS's main run loop is CAUGHT by AppKit and the app carries on, so it is not a crash and
    // there is nothing to log. Triggering there would look like this feature failing.
    Thread.detachNewThread {
        Thread.sleep(forTimeInterval: 2)
        switch shape {
        case "objc":
            NSException(name: .genericException, reason: "MAILCAL_CRASH_TEST", userInfo: nil).raise()
        case "trap":
            let empty: [Int] = []
            _ = empty[1]
        case "abort":
            abort()
        case "segv":
            UnsafeMutablePointer<Int>(bitPattern: 1)!.pointee = 0
        default:
            logAppleLifecycle("MAILCAL_CRASH_TEST=\(shape) names no shape (objc, trap, abort, segv)")
        }
    }
}
#endif
