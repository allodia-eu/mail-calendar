// Where this device remembers what it has synced with the Allodia account service. Its Android,
// Windows and Linux twins keep the same blob in each platform's own ordinary preferences.
//
// UserDefaults and not the Keychain: nothing in the blob is secret (a record id, a version, a
// fingerprint, a flag), and a Keychain prompt in front of a pass nobody started would be a prompt
// nobody is there to answer. `AppPrefs.defaults` is already per-namespace in DEBUG, so a dev
// launch's bookkeeping cannot land in the installed app's.

import Foundation
import MailcalBindings

/// The blob, in one preference, written whole.
final class UserDefaultsSyncStateStore: SyncStateStore {
    private let key = "allodia.syncState"

    func load() throws -> String? {
        AppPrefs.defaults.string(forKey: key)
    }

    func save(blob: String) throws {
        AppPrefs.defaults.set(blob, forKey: key)
    }
}

extension DevNamespace {
    /// Whether this launch connected canned harness accounts rather than the person's own.
    ///
    /// Such a launch must not sync: its accounts belong to the local test server, and sending them
    /// up would put a harness mailbox on the developer's own phone.
    static var usesCannedAccounts: Bool {
        #if DEBUG
        switch ProcessInfo.processInfo.environment["MAILCAL_DEV_ACCOUNT"] {
        case "stalwart", "stalwart-multi", "stalwart-imap": true
        default: false
        }
        #else
        false
        #endif
    }
}
