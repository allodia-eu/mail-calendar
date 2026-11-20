// The new-mail notifications choice an earlier version of this client kept in `UserDefaults`,
// handed to the core, which holds it now (`NotificationsPage`). Every path that opens the
// persisted store runs it right after the core is built and before any pass, so somebody who
// turned notifications off is not announced to by the first pass after the update. The in-memory
// demo and showcase cores do not run it: they would take the key and store the choice nowhere.
import Foundation
import MailcalBindings

enum NotificationChoiceImport {
    static let key = "new_mail_notifications_enabled"

    /// What a stored choice asks of the core: turning notifications off when it was off, and
    /// nothing when it was on, which is the core's own default.
    static func intent(storedOn: Bool) -> PreferenceIntent? {
        storedOn ? nil : .setNewMailNotifications(on: false)
    }

    /// Hands the stored choice to `app`, if there is one, and removes it from `defaults`. Waits
    /// for the core to write the choice before the key goes, because the background pass's core
    /// is released, and its process may be suspended, as soon as the pass ends. Not for a UI
    /// thread.
    static func run(on app: MailcalApp, defaults: UserDefaults = AppPrefs.defaults) {
        run(defaults: defaults) { intent in
            app.dispatch(intent: .preferences(intent: intent))
            app.finishSavingSettings()
        }
    }

    /// The import over any sender, so a test can watch what it sends without a core.
    static func run(defaults: UserDefaults, send: (PreferenceIntent) -> Void) {
        guard defaults.object(forKey: key) != nil else { return }
        if let intent = intent(storedOn: defaults.bool(forKey: key)) { send(intent) }
        defaults.removeObject(forKey: key)
    }
}
