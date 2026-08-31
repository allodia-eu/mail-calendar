// Raises local new-mail notifications on iOS/iPadOS from a background-sync outcome
// (docs/background-sync.md). One notification per account, sender + subject in the body; the OS
// hides the preview on the lock screen per the user's system setting. This deliberately shows
// content (the user chose it), distinct from the never-log-content diagnostic-log rule.
#if os(iOS)
import Foundation
import MailcalBindings
import UserNotifications

enum MailNotifier {
    /// Requests notification authorisation (alerts + sound + badge). Safe to call every launch:
    /// iOS only prompts once, then this is a no-op. Called at launch so a granting user gets
    /// background new-mail notifications.
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

    /// Posts one notification PER MESSAGE, keyed by the message's stable provider key and grouped
    /// per account via `threadIdentifier`. Keying by message (not account) is deliberate: a later
    /// background pass reports different messages, so its notifications never REPLACE an earlier
    /// still-unseen one, the high-water-mark advances past reported mail, so a clobbered
    /// notification would otherwise be lost forever. A no-op if nothing arrived or authorisation was
    /// not granted.
    static func notifyNewMail(_ outcome: BackgroundSyncOutcome) async {
        guard !outcome.accounts.isEmpty else { return }
        let center = UNUserNotificationCenter.current()
        let status = await center.notificationSettings().authorizationStatus
        guard status == .authorized || status == .provisional else { return }
        // The app-icon badge is app-wide, so every notification carries the same pass total (the
        // sum across accounts) rather than one account's count clobbering the others'.
        let passTotal = outcome.accounts.reduce(0) { $0 + Int($1.newCount) }
        for account in outcome.accounts {
            for message in account.messages {
                let request = UNNotificationRequest(
                    identifier: "new-mail-\(message.messageKey)",
                    content: content(for: message, account: account, badge: passTotal),
                    trigger: nil
                )
                try? await center.add(request)
            }
        }
    }

    private static func content(
        for message: NewMailPreview,
        account: AccountNewMail,
        badge: Int
    ) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = sender(message)
        content.body = message.subject
        content.subtitle = account.accountLabel
        content.sound = .default
        // iOS collapses same-thread notifications into one expandable stack per account.
        content.threadIdentifier = account.accountId
        content.badge = NSNumber(value: badge)
        return content
    }

    /// The display name when present, else the bare address.
    private static func sender(_ preview: NewMailPreview) -> String {
        if let name = preview.senderName, !name.isEmpty { return name }
        return preview.sender
    }
}
#endif
