// The Diagnostics settings surface, without the UI: the pure parts a wrong build would get
// wrong silently. The snapshot math decides what "Log size" and "Archived files" claim, the
// debug-toggle mapping decides what level every core construction boots with, and the line
// splitter decides what the viewer shows, each is a plain function, so each is provable here.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct DiagnosticsTests {

    // MARK: Snapshot math (what the status rows claim)

    /// A scratch directory with a fake log store: the current file plus the given backups,
    /// each written with a body of the stated byte count.
    private func makeStore(current: Int?, backups: [Int: Int]) throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("diag-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let base = dir.appendingPathComponent("mailcal.log")
        if let current {
            try Data(repeating: 0x61, count: current).write(to: base)
        }
        for (index, size) in backups {
            let url = dir.appendingPathComponent("mailcal.log.\(index)")
            try Data(repeating: 0x61, count: size).write(to: url)
        }
        return base
    }

    @Test func snapshotSumsTheCurrentFileAndEveryBackup() throws {
        // The user-facing "Log size" is the whole store, current + rotated, because that is
        // what the ~4 MB cap note is about. A size that ignored backups would understate 4x.
        let base = try makeStore(current: 100, backups: [1: 20, 2: 30])
        let snapshot = LogStoreSnapshot.measure(base: base, backups: 3)
        #expect(snapshot.totalBytes == 150)
        #expect(snapshot.backupCount == 2)
        #expect(snapshot.path == base.path)
    }

    @Test func aMissingLogIsZeroNotAnError() throws {
        // Before the first write (or on a broken install) nothing exists. The status rows must
        // say "0", never crash or throw, diagnostics may not take the app down.
        let base = try makeStore(current: nil, backups: [:])
        let snapshot = LogStoreSnapshot.measure(base: base, backups: 3)
        #expect(snapshot.totalBytes == 0)
        #expect(snapshot.backupCount == 0)
    }

    @Test func snapshotStopsAtTheConfiguredBackupCount() throws {
        // Rotation never produces a `.4`, so a stray one (a hand-copied file, an old build) is
        // not part of the store and must not inflate the count or the size.
        let base = try makeStore(current: 10, backups: [1: 10, 4: 999])
        let snapshot = LogStoreSnapshot.measure(base: base, backups: 3)
        #expect(snapshot.totalBytes == 20)
        #expect(snapshot.backupCount == 1)
    }

    @Test func aGapInTheBackupChainStillCountsLaterFiles() throws {
        // `.2` can exist without `.1` right after a rotation raced a deletion; count what is
        // actually on disk, not what a contiguous chain would predict.
        let base = try makeStore(current: 10, backups: [2: 30])
        let snapshot = LogStoreSnapshot.measure(base: base, backups: 3)
        #expect(snapshot.totalBytes == 40)
        #expect(snapshot.backupCount == 1)
    }

    // MARK: The debug toggle ↔ core log level mapping

    @Test func theDebugToggleMapsOnToDebugAndOffToInfo() {
        // ON is the support-session opt-in; OFF is the contract's INFO default
        // (docs/logging.md). This mapping is what every core-construction site boots with.
        #expect(DiagnosticsPrefs.logLevel(debugEnabled: true) == LogLevel.debug)
        #expect(DiagnosticsPrefs.logLevel(debugEnabled: false) == LogLevel.info)
    }

    // MARK: The viewer's line source

    @Test func logLinesDropOnlyTheTrailingTerminator() {
        // Every appended line ends in `\n`, so a naive split always yields one phantom empty
        // row at the end, but a *real* blank line inside the log must survive.
        #expect(logLines("") == [])
        #expect(logLines("a\nb\n") == ["a", "b"])
        #expect(logLines("a\nb") == ["a", "b"])
        #expect(logLines("a\n\nb\n") == ["a", "", "b"])
    }
}
