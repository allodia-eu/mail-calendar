// The user's "new-mail notifications" on/off choice, stored in UserDefaults. A client-side
// preference (the background sync still runs and advances the core's marks when off, only
// *posting* a notification is gated, so toggling off then on never floods with a backlog).
import Foundation

enum NotificationPrefs {
    private static let key = "new_mail_notifications_enabled"

    /// Whether new-mail notifications are on. Defaults to on for a fresh install.
    static var enabled: Bool {
        get {
            // Absent key ⇒ default on (a fresh install opts in until the user turns it off).
            guard AppPrefs.defaults.object(forKey: key) != nil else { return true }
            return AppPrefs.defaults.bool(forKey: key)
        }
        set { AppPrefs.defaults.set(newValue, forKey: key) }
    }
}
