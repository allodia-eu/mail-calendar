// The single place that decides what a DEBUG build keeps separate from a shipped one, the
// Keychain service, the engine store directory, and the preference domain. The macOS counterpart
// of Windows' `CredentialStore.UseDevNamespace` + `AppPaths` dev subdir, gathered into one type so
// the three can't drift apart or, worse, collide.
//
// Why this exists at all: on macOS a dev build and the installed Developer ID build share one
// login keychain, one home directory, and, because they share the bundle identifier, one
// `UserDefaults` domain. Nothing separates them by default, so a dev run reads, writes, and
// reorders the real app's state.
//
// Unlike Windows, this is NOT limited to the harness accounts: `--account personal` is the mode
// that touches real credentials and real mail, so it is exactly the mode that must be isolated.
//
// The rotating diagnostic log deliberately stays shared (`FileLog.swift` keeps
// `~/.local/share/mailcal`), matching Windows, one file diagnoses whatever ran last.

import Foundation
import MailcalBindings

/// Names a DEBUG build uses in place of the shipped app's, keyed by `MAILCAL_DEV_ACCOUNT`.
///
/// Each `dev(...)`/`dataDirName(...)` result is distinct per mode, so a JMAP harness run, an IMAP
/// harness run, and a `personal` dev run never share a store, a credential set, or a preference:
/// the property `DevNamespaceTests` pins, because a collision here is silent and corrupting.
enum DevNamespace {
    /// The shipped app's Keychain service. Only a release build ever names it.
    static let prodKeychainService = Brand.appID
    /// The shipped app's engine store directory (under `~/.local/share` on macOS).
    static let prodDataDirName = "mailcal"

    /// The Keychain service a DEBUG build stores accounts under.
    ///
    /// `first-run` is the one namespace nothing ever writes to on purpose: it exists so the screen
    /// somebody sees **once** can be seen again. Every other mode either injects a canned account
    /// or reads the `.dev` namespace a developer is already using, so neither can show a first
    /// run without emptying something somebody wanted.
    static func keychainService(for devAccount: String?) -> String {
        switch devAccount {
        case let tag?
        where tag == "stalwart" || tag == "stalwart-multi" || tag == "stalwart-imap"
            || tag == "first-run":
            "\(prodKeychainService).dev.\(tag)"
        default: "\(prodKeychainService).dev"
        }
    }

    /// The engine store directory a DEBUG build opens.
    ///
    /// The two harness names are unchanged (`docs/debugging.md` and every other client document
    /// them), so `personal` takes a third name rather than the base `mailcal-dev`, which would
    /// have silently shared a store with the JMAP harness. `demo`/showcase never reach here: they
    /// build an in-memory core with no store at all.
    static func dataDirName(for devAccount: String?) -> String {
        switch devAccount {
        case "stalwart": "mailcal-dev"
        case "stalwart-multi": "mailcal-dev-multi"
        case "stalwart-imap": "mailcal-dev-imap"
        case "first-run": "mailcal-dev-first-run"
        default: "mailcal-dev-personal"
        }
    }

    /// The absolute directory the engine store, `preferences.toml` and the credential index live
    /// in for `name`. macOS keeps them under the user's home; iOS has no user home, so they go in
    /// the app's Application Support.
    ///
    /// One place, so a host that has to read a preference *before* the core exists, the light/dark
    /// appearance is wanted before the first frame, cannot name a different directory from the one
    /// the core will open.
    static func storeDirectory(_ name: String = currentDataDirName) -> String {
        #if os(macOS)
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/share/\(name)").path
        #else
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(name).path
        #endif
    }

    #if DEBUG
    /// This process's mode, read once, `BackgroundSync` can reach the store from a BGTask before
    /// `MailboxModel.start()` has run, so nothing here may depend on startup ordering.
    private static let devAccount = ProcessInfo.processInfo.environment["MAILCAL_DEV_ACCOUNT"]
    static let currentKeychainService = keychainService(for: devAccount)
    static let currentDataDirName = dataDirName(for: devAccount)
    #else
    static let currentKeychainService = prodKeychainService
    static let currentDataDirName = prodDataDirName
    #endif
}

/// Where client-side preferences live. `UserDefaults.standard` is keyed by the bundle identifier,
/// which a dev build shares with the installed app, so in DEBUG the preferences move to their own
/// suite, the macOS twin of Windows moving its one-line preference files into the dev subdir.
///
/// Known gap: SwiftUI's `@SceneStorage` (window/scene restoration) and `NSSplitView`'s autosave are
/// written by the frameworks into the app's own domain and saved-state bundle, which cannot be
/// redirected this way; `SplitViewAutosave`'s name is varied per namespace instead, and scene
/// restoration is still shared. See `docs/debugging.md`.
enum AppPrefs {
    // `nonisolated(unsafe)`, not a lock or an actor: `UserDefaults` does its own synchronisation
    // and is documented thread-safe, it simply predates `Sendable` and cannot say so.
    #if DEBUG
    nonisolated(unsafe) static let defaults =
        UserDefaults(suiteName: DevNamespace.currentKeychainService) ?? .standard
    #else
    nonisolated(unsafe) static let defaults = UserDefaults.standard
    #endif

    /// Suffixes an AppKit autosave name so a dev build doesn't restore (or overwrite) the
    /// installed app's saved layout.
    static func autosaveName(_ base: String) -> String {
        #if DEBUG
        "\(base)-\(DevNamespace.currentDataDirName)"
        #else
        base
        #endif
    }
}
