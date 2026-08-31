// What a MetricKit report writes into the log. The delivery itself cannot be driven from a test:
// only the system produces a payload, at a launch after the one that died, so what a gate can
// reach is the wording, and the one rule that keeps these records apart from live ones.

import Foundation
import Testing

@testable import MailcalUI

@Suite struct CrashDiagnosticsTests {

    @Test func aReportAboutAnEarlierSessionMustNotReadAsALiveCrash() {
        // `unhandled` is the string support greps across four clients for "the process is dying
        // now". These records are about a session that ended, possibly days ago and possibly in a
        // different build, spending the same word on both would make every search return two
        // kinds of thing, and the reader cannot tell them apart afterwards.
        let crash = CrashDiagnostics.crashRecord(
            build: "202608261200", reason: "Namespace SIGNAL, Code 11", exceptionType: 1, signal: 11
        )
        let hang = CrashDiagnostics.hangRecord(build: "202608261200", seconds: 4.25)

        #expect(!crash.contains("unhandled"))
        #expect(!hang.contains("unhandled"))
        #expect(crash.contains("ended in a crash"))
        #expect(hang.contains("unresponsive"))
    }

    @Test func theRecordNamesTheBuildThatDiedRatherThanTheOneReadingIt() {
        // The session marker names the build that is *running*. A payload arrives at the next
        // launch, which may well be a launch of a newer build, updating is a common thing to do
        // about a crash, so without this the report is pinned to the wrong version.
        let record = CrashDiagnostics.crashRecord(
            build: "202608010900", reason: nil, exceptionType: nil, signal: nil
        )

        #expect(record.contains("build 202608010900"))
    }

    @Test func aSignalIsNamedTheSameWayTheLiveHandlerNamesIt() {
        // Both halves of the Apple crash story read from `CrashLog.watchedSignals`, so a reader
        // grepping SIGSEGV finds the live record and the retrospective one alike.
        #expect(CrashDiagnostics.signalName(Int(SIGSEGV)) == "SIGSEGV")
        #expect(CrashDiagnostics.signalName(Int(SIGABRT)) == "SIGABRT")
    }

    @Test func aSignalWeDoNotNameStillReportsItsNumber() {
        // MetricKit reports whatever killed the process, which is a wider set than the six
        // `CrashLog` arms handlers for. Falling back to the number keeps the report truthful
        // instead of dropping the one fact it had.
        #expect(CrashDiagnostics.signalName(Int(SIGKILL)) == "\(SIGKILL)")
    }

    @Test func aCrashWithNothingKnownAboutItStillWritesALine() {
        // Every field on MXCrashDiagnostic is optional. A record that composed itself into
        // "ended in a crash: " and stopped would look like a bug in the log rather than a crash
        // the system declined to describe.
        let record = CrashDiagnostics.crashRecord(
            build: "unknown", reason: nil, exceptionType: nil, signal: nil
        )

        #expect(record.hasSuffix("no detail given"))
    }

    @Test func aHangReportsSecondsToOneDecimal() {
        // The difference between a 2s hang and a 20s one is the whole diagnosis; more precision
        // than a tenth changes nothing and just makes the line harder to scan.
        #expect(CrashDiagnostics.hangRecord(build: "b", seconds: 4.27).contains("4.3s"))
        #expect(CrashDiagnostics.hangRecord(build: "b", seconds: 19.99).contains("20.0s"))
    }

    @Test func theDecimalSeparatorIsADotInEveryLanguageTheAppShips() {
        // This is a log line, read and grepped by whoever picks up the support request, in an app
        // that ships in seven languages. `String(format:)` is unlocalized, a switch to
        // NumberFormatter would quietly write "4,3" for a Dutch or German user and the line would
        // still look right to the person who made the change.
        let record = CrashDiagnostics.hangRecord(build: "b", seconds: 4.27)

        #expect(record.contains("4.3s"))
        #expect(!record.contains(","))
    }
}
