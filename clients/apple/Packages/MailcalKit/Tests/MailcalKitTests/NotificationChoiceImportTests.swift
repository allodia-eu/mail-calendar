// The new-mail notifications choice an earlier version kept in `UserDefaults`, handed to the core
// once: what it sends for each stored value, and that the key is gone afterwards so it is never
// sent again over a choice the person has since changed in the core.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct NotificationChoiceImportTests {
    /// Runs `body` over a preference domain of its own, removed afterwards, so a test neither
    /// reads the app's preferences nor leaves any behind.
    private func withScratchDefaults(_ body: (UserDefaults) throws -> Void) throws {
        let suite = "notification-import-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        try body(defaults)
    }

    private func runImport(_ defaults: UserDefaults) -> [PreferenceIntent] {
        var sent: [PreferenceIntent] = []
        NotificationChoiceImport.run(defaults: defaults) { sent.append($0) }
        return sent
    }

    @Test func aStoredOffTurnsTheCoresChoiceOff() throws {
        try withScratchDefaults { defaults in
            defaults.set(false, forKey: NotificationChoiceImport.key)
            #expect(runImport(defaults) == [.setNewMailNotifications(on: false)])
            #expect(defaults.object(forKey: NotificationChoiceImport.key) == nil)
        }
    }

    @Test func aStoredOnSendsNothingAndIsRemoved() throws {
        // On is the core's default, so there is nothing to change; the key still goes.
        try withScratchDefaults { defaults in
            defaults.set(true, forKey: NotificationChoiceImport.key)
            #expect(runImport(defaults).isEmpty)
            #expect(defaults.object(forKey: NotificationChoiceImport.key) == nil)
        }
    }

    @Test func noStoredChoiceSendsNothing() throws {
        try withScratchDefaults { defaults in
            #expect(runImport(defaults).isEmpty)
            #expect(defaults.object(forKey: NotificationChoiceImport.key) == nil)
        }
    }

    @Test func aSecondRunSendsNothing() throws {
        // Every launch runs the import; only the first after the update may find a choice.
        try withScratchDefaults { defaults in
            defaults.set(false, forKey: NotificationChoiceImport.key)
            #expect(runImport(defaults) == [.setNewMailNotifications(on: false)])
            #expect(runImport(defaults).isEmpty)
        }
    }

    @Test func theKeyIsTheOneEarlierVersionsWrote() {
        // A different spelling would find nothing and leave notifications on for somebody who
        // had turned them off.
        #expect(NotificationChoiceImport.key == "new_mail_notifications_enabled")
    }
}
