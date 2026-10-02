// Raises local new-mail notifications from a background-sync outcome (docs/background-sync.md).
// One notification per message: the sender as its title, then the subject and how the message
// begins; the OS hides all of it on the lock screen per the user's system setting. This
// deliberately shows content (the user chose it), distinct from the never-log-content
// diagnostic-log rule.
//
// What each notification says is `NewMailNotices`; this posts it, and answers the click. Both
// Apple platforms arrive here, from the two mechanisms their processes allow: iOS from the
// `BGAppRefreshTask` (BackgroundSync.swift), macOS from the live runtime's own mailbox signal
// (MailcalModel.Notifications.swift).
import Foundation
import MailcalBindings
import UserNotifications

/// The `userInfo` keys a notification carries its message on, read back in `didReceive`.
private enum NoticeKey {
    static let account = "mailcal.account"
    static let message = "mailcal.messageKey"
}

enum MailNotifier {
    /// Requests notification authorisation (alerts + sound + badge). Safe to call every launch:
    /// the OS only prompts once, then this is a no-op. Called at launch so a granting user gets
    /// new-mail notifications.
    static func requestAuthorization() {
        UNUserNotificationCenter.current()
            .requestAuthorization(options: [.alert, .sound, .badge]) { _, error in
                if let error {
                    FileLog.shared.append(
                        level: "WARN",
                        target: "background",
                        message: "notification auth failed: \(error)"
                    )
                }
            }
    }

    /// Installs the delegate that answers a click, and in a DEBUG build also presents a banner
    /// while the app is frontmost. Idempotent, so every launch edge may call it.
    ///
    /// Not DEBUG-only: without a delegate the OS default-action just activates the app, which is
    /// the behaviour the deep link exists to replace.
    static func installDelegate() {
        UNUserNotificationCenter.current().delegate = NotificationDelegate.shared
    }

    /// Posts one notification PER MESSAGE, keyed by the message's stable provider key and grouped
    /// per account via `threadIdentifier`. Keying by message (not account) is deliberate: a later
    /// pass reports different messages, so its notifications never REPLACE an earlier still-unseen
    /// one, the high-water-mark advances past reported mail, so a clobbered notification would
    /// otherwise be lost forever. A no-op if nothing arrived or authorisation was not granted.
    static func notifyNewMail(_ outcome: BackgroundSyncOutcome) async {
        guard !outcome.accounts.isEmpty else { return }
        let center = UNUserNotificationCenter.current()
        let status = await center.notificationSettings().authorizationStatus
        guard status == .authorized || status == .provisional else { return }
        for notice in NewMailNotices.forPass(outcome) {
            let request = UNNotificationRequest(
                identifier: notice.identifier, content: content(notice), trigger: nil)
            try? await center.add(request)
        }
    }

    private static func content(_ notice: NewMailNotice) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = notice.title
        content.subtitle = notice.subtitle
        content.body = notice.body
        content.sound = .default
        // The OS collapses same-thread notifications into one expandable stack per account.
        content.threadIdentifier = notice.threadIdentifier
        if let badge = notice.badge { content.badge = NSNumber(value: badge) }
        if let target = notice.target {
            content.userInfo = [NoticeKey.account: target.account, NoticeKey.message: target.key]
        }
        return content
    }

    /// The message a delivered notification names, or `nil` where it names none (a summary).
    static func target(of notification: UNNotification) -> NewMailTarget? {
        let info = notification.request.content.userInfo
        guard let account = info[NoticeKey.account] as? String, !account.isEmpty,
              let key = info[NoticeKey.message] as? String, !key.isEmpty
        else { return nil }
        return NewMailTarget(account: account, key: key)
    }
}

/// Answers what the user does with a notification.
///
/// `@unchecked Sendable`, like `DiagnosticSink`: `UNUserNotificationCenterDelegate` is not
/// main-actor bound, and this holds no stored property for a delivery to race against.
final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    static let shared = NotificationDelegate()

    /// A click or tap. The default action is the one on the banner itself; an action a future
    /// build adds would arrive here under its own identifier and is deliberately not treated as
    /// one. The message goes in the box the shell drains, because the delegate is process-wide
    /// while the reading pane belongs to a scene (Mailcal.NotificationOpen.swift).
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        guard response.actionIdentifier == UNNotificationDefaultActionIdentifier,
              let target = MailNotifier.target(of: response.notification)
        else { return }
        NotificationOpenInbox.put(target)
    }

    #if DEBUG
    /// DEBUG-only: presents new-mail notifications even while the app is frontmost, so a live test
    /// can see the banner at all. On iOS the simulator cannot background the app; on macOS the app
    /// under test is the window being watched. A release build keeps the default, foreground
    /// notifications are suppressed, since you do not notify somebody about mail they are looking
    /// at.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound, .list]
    }
    #endif
}
