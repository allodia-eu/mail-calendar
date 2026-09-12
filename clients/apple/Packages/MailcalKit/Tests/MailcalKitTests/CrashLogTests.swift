// The crash log's composition, and the set of signals it claims. There is no Apple UI-test
// target, so this is what a gate can reach: the wording, and the choice of which faults are
// caught. Arming the handlers and actually dying is verified by hand with
// `MAILCAL_CRASH_TEST=objc|trap|abort|segv` (docs/logging.md).

import Foundation
import Testing

@testable import MailcalUI

@Suite struct CrashLogTests {

    // MARK: What an uncaught NSException writes

    @Test func theRecordLeadsWithTheWordEveryPlatformGreps() {
        let record = CrashLog.record(
            name: "NSInvalidArgumentException",
            reason: "quorrix went sideways",
            frames: ["0   MailcalUI    0x0 thing", "1   AppKit    0x0 other"]
        )

        #expect(record.hasPrefix("unhandled NSInvalidArgumentException: quorrix went sideways\n"))
        #expect(record.contains("0   MailcalUI"))
        #expect(record.contains("1   AppKit"))
    }

    @Test func anExceptionWithNoReasonStillWritesALine() {
        // `NSException.reason` is optional, and a crash record that threw while composing itself
        // would take the app down in place of the fault it was reporting.
        let record = CrashLog.record(name: "NSGenericException", reason: nil, frames: [])

        #expect(record == "unhandled NSGenericException: no reason")
    }

    // MARK: What a fatal signal writes

    @Test func theSignalBannerNamesTheSignalAndCarriesTheSharedToken() {
        let banner = CrashLog.banner(forSignalNamed: "SIGSEGV")

        #expect(banner.contains("unhandled signal SIGSEGV"))
        // Its own line at both ends: the record is appended straight after whatever was mid-write,
        // and the frames follow immediately below.
        #expect(banner.hasPrefix("\n"))
        #expect(banner.hasSuffix("\n"))
    }

    // MARK: Which faults are claimed

    @Test func everyFaultThatKillsThisAppIsCaught() {
        let caught = Set(CrashLog.watchedSignals.map(\.number))

        // A Swift trap, force-unwrap nil, index out of range, fatalError, precondition, is
        // SIGILL on x86_64 and SIGTRAP on arm64, so both are required for one build to cover both
        // architectures. SIGABRT and SIGSEGV/SIGBUS are the cdylib's native deaths.
        for required in [SIGILL, SIGTRAP, SIGABRT, SIGSEGV, SIGBUS, SIGFPE] {
            #expect(caught.contains(required), "signal \(required) is not caught")
        }
    }

    @Test func nothingThatIsNotACrashIsCaught() {
        let caught = Set(CrashLog.watchedSignals.map(\.number))

        // SIGPIPE is ignored by Foundation and by Rust on purpose; SIGINT/SIGTERM/SIGHUP are how a
        // process is asked to stop. Catching any of them would file an ordinary quit as a crash:
        // which is the exact confusion this feature exists to remove, in reverse.
        for polite in [SIGPIPE, SIGINT, SIGTERM, SIGHUP, SIGQUIT] {
            #expect(!caught.contains(polite), "signal \(polite) must not be treated as a crash")
        }
    }

    @Test func nothingIsSuppressedUntilACrashActuallyClaimsTheLog() {
        // FileLog stands down while a signal handler is writing, so that another thread's line
        // cannot land in the middle of a stack. If that flag ever read `true` at rest the app
        // would log NOTHING, for the whole session, with no error anywhere, so its resting value
        // is worth an assertion even though the crash side of it can only be driven by hand.
        #expect(!isWritingCrashRecord())
    }

    @Test func everySignalHasABannerSlotItCanBeIndexedBy() {
        // The banners are held in a fixed 32-slot table so the handler can index into it without
        // allocating. A signal number outside that range would silently get no banner.
        for watched in CrashLog.watchedSignals {
            #expect(watched.number > 0 && watched.number < 32, "\(watched.name) has no slot")
            #expect(!watched.name.isEmpty)
        }
    }
}
