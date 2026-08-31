// Which light/dark appearance a launch paints in.
//
// The choice itself is a CORE setting (docs/settings.md → General), persisted in preferences.toml
// beside every other display preference, so the clients cannot each invent their own default. It is
// read straight off disk rather than pulled from `MailcalApp` because the first frame is drawn
// before the core exists: `newAccounts` opens the engine store and starts dialing, and a window
// painted in the desktop's scheme until that returns is a visible flash of exactly the theme the
// user said they did not want.
//
// `launchOverride` is hard-`nil` outside DEBUG, so a shipped app cannot have its theme flipped by a
// stray environment variable, the property the dev-account and showcase switches also hold.

import Foundation
import MailcalBindings
import SwiftUI

/// Resolves the appearance the app comes up in, and turns one into a SwiftUI scheme.
enum AppearanceMode {
    /// The appearance this launch paints with: the `MAILCAL_APPEARANCE` override when it names one,
    /// else the core's persisted choice. A later pick in Settings wins for the rest of the session:
    /// the override decides how a run *starts*, not what the app may do.
    static func atLaunch() -> Appearance {
        launchOverride ?? storedAppearance(dataDir: DevNamespace.storeDirectory())
    }

    static var launchOverride: Appearance? {
        #if DEBUG
        parse(ProcessInfo.processInfo.environment["MAILCAL_APPEARANCE"])
        #else
        nil
        #endif
    }

    /// The cross-client spellings, matched literally. Anything else is ignored rather than read as
    /// "system": a typo'd override that silently did nothing looks exactly like a working one in the
    /// screenshot it was meant to shape.
    static func parse(_ raw: String?) -> Appearance? {
        switch raw?.trimmingCharacters(in: .whitespaces).lowercased() {
        case "system": .system
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }

    /// The scheme to force on the view hierarchy, or `nil` to leave it following the host, which is
    /// what keeps a desktop light/dark switch reaching the app while it runs.
    static func colorScheme(_ appearance: Appearance) -> ColorScheme? {
        switch appearance {
        case .light: .light
        case .dark: .dark
        case .system: nil
        }
    }
}
