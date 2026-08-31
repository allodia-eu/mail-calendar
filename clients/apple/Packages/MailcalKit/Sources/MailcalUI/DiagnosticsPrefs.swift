// The user's "include more detail" (DEBUG) diagnostics choice, stored in UserDefaults. A
// client-side preference, mirroring NotificationPrefs: the core owns the *current* level via
// `setLogLevel`, but forgets it on relaunch, so the choice is persisted here and re-applied as
// the `logLevel:` argument at every core-construction site (foreground boot, the iOS background
// refresh's cold worker, the debug/dev boots). OFF is the contract's INFO default
// (docs/logging.md), which keeps the rotating file log useful over a long window.
import Foundation
import MailcalBindings

enum DiagnosticsPrefs {
    private static let key = "diagnostic_log_debug_enabled"

    /// Whether the DEBUG opt-in is on. Defaults to off, INFO is the contract's default level,
    /// and an absent key reads as `false`.
    static var debugLogging: Bool {
        get { AppPrefs.defaults.bool(forKey: key) }
        set { AppPrefs.defaults.set(newValue, forKey: key) }
    }

    /// The persisted choice as the core's `LogLevel`, what every core construction passes as
    /// `logLevel:` instead of a hard-coded `.info`.
    static var coreLogLevel: LogLevel { logLevel(debugEnabled: debugLogging) }

    /// The pure mapping the toggle and the boot sites share: ON opts into the core's per-phase
    /// DEBUG detail for a support session, OFF is the INFO default.
    static func logLevel(debugEnabled: Bool) -> LogLevel {
        debugEnabled ? .debug : .info
    }
}
