// What the log says about a session that died without getting to say anything itself.
//
// `CrashLog` covers the deaths the process can narrate on its way out. Three shapes it cannot:
//
//   * the system kills the app for memory pressure (jetsam), no signal is delivered
//   * the watchdog kills it for being unresponsive, likewise
//   * the fault is violent enough that the handler itself does not finish
//
// MetricKit reports all three, but only at the NEXT launch, so this is retrospective by
// construction: it is a line about a session that has already ended. That is exactly why these
// records do **not** carry `unhandled`, that word is the one string support greps across four
// clients for "the process is dying now", and spending it on a report about last Tuesday would
// make every search return two kinds of thing. The same rule the Linux client applies to a GLib
// critical (clients/linux/src/crash.rs).
//
// Only the headline is written, never the call-stack tree. That tree is unsymbolicated binary
// offsets, useful offline, useless in a support log, and easily large enough to evict the very
// history it was appended to from the rotating cap. For the shapes above there is no stack worth
// having anyway: nothing crashed, the app was taken away. The OS crash report holds the rest.

import Foundation
import MetricKit

/// Reports what MetricKit says about earlier sessions into the diagnostic log.
public enum CrashDiagnostics {
    /// Subscribes for the life of the process. Called from `AllodiaApp.init()` beside
    /// `CrashLog.install()`, and like it, only after `FileLog` has a file to write to.
    public static func watchForEndedSessions() {
        MXMetricManager.shared.add(sink)
    }

    /// Held strongly here because `MXMetricManager` does not retain its subscribers; a sink that
    /// went out of scope would take every future report with it, silently.
    private static let sink = DiagnosticSink()

    /// The line a crash report from an earlier session writes.
    ///
    /// `build` leads it, and that is the point of writing this at all rather than trusting the
    /// session marker: a payload is delivered at the *next* launch, which may be a launch of a
    /// **different build**, the user updated, which is often what they did about the crash. The
    /// marker names the build that is running now; only this names the one that died.
    static func crashRecord(build: String, reason: String?, exceptionType: Int?, signal: Int?)
        -> String
    {
        var facts: [String] = []
        if let signal { facts.append("signal \(signalName(signal))") }
        if let exceptionType { facts.append("exception type \(exceptionType)") }
        if let reason, !reason.isEmpty { facts.append(reason) }
        let detail = facts.isEmpty ? "no detail given" : facts.joined(separator: ", ")
        return "an earlier session (build \(build)) ended in a crash: \(detail)"
    }

    /// The line an unresponsive stretch writes.
    ///
    /// A hang is the shape a user reports as "it froze" and the one shape no crash handler can
    /// ever see, because nothing crashed. Seconds to one decimal: the difference between a 2s hang
    /// and a 20s one is the whole diagnosis, and no more precision than that changes anything.
    static func hangRecord(build: String, seconds: Double) -> String {
        "an earlier session (build \(build)) was unresponsive for "
            + String(format: "%.1f", seconds) + "s"
    }

    /// The name for a signal number, or the number when it is one we do not name.
    ///
    /// Reuses the table `CrashLog` arms its handlers from, so the two halves of the Apple crash
    /// story cannot drift into calling the same signal different things.
    static func signalName(_ number: Int) -> String {
        CrashLog.watchedSignals.first { Int($0.number) == number }?.name ?? "\(number)"
    }
}

/// The MetricKit subscriber itself. Separate from the enum above because MetricKit requires an
/// `NSObject`, and because nothing outside this file has any reason to hold one.
private final class DiagnosticSink: NSObject, MXMetricManagerSubscriber {
    /// Required by the protocol, and deliberately empty: these are the daily power and performance
    /// aggregates, which are a product-analytics question and not a diagnostic-log one. Analytics
    /// here is opt-in, EU-only and closed-enum by construction (docs/analytics.md); quietly writing
    /// device metrics into a file the user shares would go around all three.
    func didReceive(_ payloads: [MXMetricPayload]) {}

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        for payload in payloads {
            let build = payload.crashDiagnostics?.first?.metaData.applicationBuildVersion
                ?? payload.hangDiagnostics?.first?.metaData.applicationBuildVersion
                ?? "unknown"
            for crash in payload.crashDiagnostics ?? [] {
                FileLog.shared.append(
                    level: "ERROR",
                    target: "crash",
                    message: CrashDiagnostics.crashRecord(
                        build: build,
                        reason: crash.terminationReason,
                        exceptionType: crash.exceptionType?.intValue,
                        signal: crash.signal?.intValue
                    )
                )
            }
            for hang in payload.hangDiagnostics ?? [] {
                FileLog.shared.append(
                    level: "WARN",
                    target: "crash",
                    message: CrashDiagnostics.hangRecord(
                        build: build,
                        seconds: hang.hangDuration.converted(to: .seconds).value
                    )
                )
            }
        }
    }
}
