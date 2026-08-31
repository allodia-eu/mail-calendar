// A size-rotating file log for the shared Apple client, the counterpart of the Windows
// client's file Log (`Services/Log.cs`). The core's diagnostics (routed through the FFI
// `Logger`) also go to os_log (Console.app), but a file survives a force-quit and is trivial
// to grab, which matters when the UI hangs.
//
// Rotation is size-based, at 1 MB, mailcal.log -> mailcal.log.1 -> ... -> mailcal.log.3
// (oldest dropped), so the logs cap at ~4 MB total and never grow unbounded. See the
// cross-platform contract in docs/logging.md.

import Foundation
import MailcalBindings

/// Serialized, size-rotating log file. `append` is called from arbitrary Rust runtime
/// threads, so writes (and the rotation check) hop onto one serial queue to avoid interleaving.
final class FileLog {
    static let shared = FileLog()

    private static let maxBytes: UInt64 = 1 << 20 // 1 MB per file
    private static let backups = 3                // mailcal.log + .1..3 => ~4 MB cap

    private let queue = DispatchQueue(label: "\(Brand.appID).filelog")
    private let url: URL
    private let formatter: DateFormatter

    private init() {
        #if os(macOS)
        let dir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/share/mailcal")
        #else
        // iOS has no user home; the diagnostic log lives in the app's Application Support.
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("mailcal")
        #endif
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        url = dir.appendingPathComponent("mailcal.log")
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        let os = ProcessInfo.processInfo.operatingSystemVersionString
        // The app version and build number lead the banner, because a log attached to a support
        // request is otherwise unattributable: `/VERSION` holds the last *released* version, so a
        // dev build and a shipped one report the same marketing version and only the build number
        // (a fresh dotted UTC timestamp per package, docs/versioning.md) tells them apart.
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "0.0.0"
        let build = info?["CFBundleVersion"] as? String ?? "0"
        write(
            "\(formatter.string(from: Date())) INFO [filelog] "
                + "--- session start (\(short) build \(build), \(os)) ---\n"
        )
    }

    /// Appends one line: `<timestamp> <LEVEL> [target] message`.
    func append(level: String, target: String, message: String) {
        let ts = formatter.string(from: Date())
        write("\(ts) \(level) [\(target)] \(message)\n")
    }

    /// The same, but it does not return until the bytes are on disk.
    ///
    /// For the one caller that has no later moment: an uncaught NSException (`CrashLog`) is the
    /// app's last words, and the ordinary `append` hands the line to a serial queue and returns:
    /// so the process would be gone before the write ran, and the crash would leave exactly the
    /// silence this whole feature removes.
    func appendNow(level: String, target: String, message: String) {
        let ts = formatter.string(from: Date())
        queue.sync { self.writeLine("\(ts) \(level) [\(target)] \(message)\n") }
    }

    /// The current log file's location, what "Share log" hands to the share sheet and what
    /// "Copy path" copies. Immutable after init, so it needs no queue hop.
    var fileURL: URL { url }

    /// A point-in-time view of the log store (path, total size, backup count), taken on the
    /// write queue so a rotation can never run mid-measure. Best-effort, like every other
    /// FileLog operation.
    func snapshot() -> LogStoreSnapshot {
        queue.sync { LogStoreSnapshot.measure(base: url, backups: Self.backups) }
    }

    /// The current log file's full text (newest last) for the in-app viewer, read on the write
    /// queue so it never tears mid-append. Unreadable or missing reads as empty, diagnostics
    /// must never take the app down.
    func readCurrentLog() -> String {
        queue.sync { (try? String(contentsOf: url, encoding: .utf8)) ?? "" }
    }

    /// Serializes the write, rolling the file first if it has hit the size cap. Best-effort:
    /// a failed rotate or write is swallowed, logging must never take the app down.
    private func write(_ line: String) {
        queue.async { self.writeLine(line) }
    }

    /// The write itself. Must run on `queue`, `write` and `appendNow` are the two ways on.
    private func writeLine(_ line: String) {
        // A signal handler cannot take this queue, so it writes on its own descriptor and claims
        // the file with a flag instead. Standing down keeps another thread's line out of the
        // middle of a crash stack; the process is dying, so nothing worth reading is lost.
        guard !isWritingCrashRecord() else { return }
        rotateIfNeeded()
        guard let data = line.data(using: .utf8) else { return }
        if let handle = try? FileHandle(forWritingTo: url) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        }
    }

    /// Size-based rotation (must run on `queue`, before a write): at the cap,
    /// mailcal.log -> .1 -> ... -> .backups (oldest dropped), then a fresh mailcal.log. Each
    /// destination is vacated before its move, so a plain `moveItem` never hits an existing
    /// file. Mirrors the Windows client's `Log.Rotate`.
    private func rotateIfNeeded() {
        let fm = FileManager.default
        guard let attrs = try? fm.attributesOfItem(atPath: url.path),
              let size = (attrs[.size] as? NSNumber)?.uint64Value,
              size >= Self.maxBytes else { return }
        let base = url.path
        try? fm.removeItem(atPath: "\(base).\(Self.backups)")
        var i = Self.backups - 1
        while i >= 1 {
            let src = "\(base).\(i)"
            if fm.fileExists(atPath: src) {
                try? fm.moveItem(atPath: src, toPath: "\(base).\(i + 1)")
            }
            i -= 1
        }
        try? fm.moveItem(atPath: base, toPath: "\(base).1")
        fm.createFile(atPath: base, contents: nil)
    }
}

/// Records a native Apple client lifecycle event into the same rotating diagnostic log as the
/// Rust core. Public so the app delegate can log process-level launch/termination events.
public func logAppleLifecycle(_ message: String) {
    FileLog.shared.append(level: "INFO", target: "apple-ui", message: message)
}

/// What the Diagnostics settings' status rows state about the log store: where the current
/// file lives, how big the store is across current + backups, and how many backups exist.
struct LogStoreSnapshot: Equatable {
    /// Absolute path of the current log file (`mailcal.log`).
    let path: String
    /// Total bytes across the current file and every existing backup.
    let totalBytes: UInt64
    /// How many rotated backups (`.1` … `.<backups>`) exist right now.
    let backupCount: Int
}

extension LogStoreSnapshot {
    /// Measures the store rooted at `base` (the current log) plus up to `backups` rotated
    /// siblings. Pure filesystem arithmetic, split from `FileLog` so it is testable over a
    /// scratch directory; a missing or unreadable file counts as absent, never an error.
    static func measure(base: URL, backups: Int) -> LogStoreSnapshot {
        let fm = FileManager.default
        func bytes(atPath path: String) -> UInt64? {
            guard let attrs = try? fm.attributesOfItem(atPath: path) else { return nil }
            return (attrs[.size] as? NSNumber)?.uint64Value
        }
        var total = bytes(atPath: base.path) ?? 0
        var count = 0
        for index in stride(from: 1, through: backups, by: 1) {
            if let size = bytes(atPath: "\(base.path).\(index)") {
                total += size
                count += 1
            }
        }
        return LogStoreSnapshot(path: base.path, totalBytes: total, backupCount: count)
    }
}
