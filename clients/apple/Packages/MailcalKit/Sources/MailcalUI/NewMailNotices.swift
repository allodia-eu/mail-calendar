// What a new-mail pass becomes on screen: who the message is from, what it is about, how it
// begins, and on the desktop a summary for whatever the core's preview cap left out
// (docs/background-sync.md). The Apple twin of the Windows client's `NewMailNotices` and the Linux
// client's `notification_parts`.
//
// UserNotifications-free on purpose: what a pass says is decided here, as a pure function a host
// test can read, and `MailNotifier` only hands each notice to the notification centre. Keying by
// MESSAGE rather than by account is a contract requirement, and the way it fails is silent. A
// per-account identifier would be REPLACED by the next pass's notification, and the high-water
// mark has advanced past the earlier message by then, so it would disappear with nothing on
// screen looking wrong.

import Foundation
import MailcalBindings

/// One notification: what it says, and the identity the notification centre replaces it by.
struct NewMailNotice: Equatable {
    /// The first line: who the message is from.
    let title: String
    /// The line under the title, empty where this platform has nothing to put there.
    let subtitle: String
    /// The rest of what the notification says.
    let body: String
    /// This notification's own identity, unique per message.
    let identifier: String
    /// The account it belongs to, so the OS stacks an account's notifications together.
    let threadIdentifier: String
    /// The app-icon badge to set, `nil` where the platform carries none.
    let badge: Int?
    /// The message this notification stands for, so a click can open it. `nil` on a summary,
    /// which names no single message and therefore opens the app and nothing else.
    let target: NewMailTarget?
}

/// The message a notification opens when it is clicked: the pair the core keys a message on.
///
/// Carried on the notification itself rather than recovered from its identifier, so the format of
/// that string stays an implementation detail of whoever raises it.
struct NewMailTarget: Equatable {
    let account: String
    let key: String
}

enum NewMailNotices {
    /// The notifications one pass raises, in account order, newest message first within an
    /// account.
    static func forPass(_ outcome: BackgroundSyncOutcome) -> [NewMailNotice] {
        // The badge is app-wide, so every notification carries the same pass total rather than one
        // account's count clobbering the others'.
        let passTotal = outcome.accounts.reduce(0) { $0 + Int($1.newCount) }
        return outcome.accounts.flatMap { forAccount($0, passTotal: passTotal) }
    }

    #if os(macOS)
    private static func forAccount(
        _ account: AccountNewMail, passTotal: Int
    ) -> [NewMailNotice] {
        var notices = account.messages.map { message in
            // The subject and the snippet, in that order, into the two slots under the title, and
            // only the ones that exist: an IMAP account has no snippet until the body sync has
            // run, and a filled subtitle over an empty body reads as a message that begins with
            // nothing. A single part therefore goes in the body, which is the slot a banner always
            // draws.
            let parts = [oneLine(message.subject), oneLine(message.preview)]
                .filter { !$0.isEmpty }
            return NewMailNotice(
                title: sender(message),
                subtitle: parts.count > 1 ? parts[0] : "",
                body: parts.last ?? "",
                identifier: "new-mail-\(message.messageKey)",
                threadIdentifier: account.accountId,
                // No dock badge. The pass total is what has just arrived, not what is unread, so a
                // dock reading 3 and then 1 as passes land would say the second. Windows and Linux
                // badge nothing either; the phone does, because there the pass is the only thing
                // that ran.
                badge: nil,
                target: NewMailTarget(account: account.accountId, key: message.messageKey)
            )
        }
        // `newCount` is the pass's true total and `messages` is capped by the core, so only the
        // difference earns a summary and a pass that fitted says nothing extra.
        let hidden = Int(account.newCount) - account.messages.count
        if hidden > 0 {
            notices.append(
                NewMailNotice(
                    // Its own copy rather than the unknown-sender line: a single hidden message
                    // must not read as one more subject-less message.
                    title: L10n.notification_more_messages(count: hidden),
                    // The summary stands for messages it does not name, so it has no one body to
                    // quote and names the account instead.
                    subtitle: "",
                    body: account.accountLabel,
                    identifier: "new-mail-account-\(account.accountId)",
                    threadIdentifier: account.accountId,
                    badge: nil,
                    // It stands for messages it does not name, so there is nothing to open: a
                    // click brings the app forward and leaves the reading pane where it was.
                    target: nil
                ))
        }
        return notices
    }

    /// The subject and the snippet as the list row shows them, whitespace collapsed to single
    /// spaces, so a notification and its row cannot quote the same message differently.
    private static func oneLine(_ text: String) -> String {
        text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }
    #endif

    #if os(iOS)
    private static func forAccount(
        _ account: AccountNewMail, passTotal: Int
    ) -> [NewMailNotice] {
        var notices = account.messages.map { message in
            NewMailNotice(
                title: sender(message),
                // Which account the mail landed on, which is what a phone carrying several needs
                // first; the subject and the snippet share the body under it.
                subtitle: account.accountLabel,
                body: body(message),
                identifier: "new-mail-\(message.messageKey)",
                threadIdentifier: account.accountId,
                badge: passTotal,
                target: NewMailTarget(account: account.accountId, key: message.messageKey)
            )
        }
        // `newCount` is the pass's true total and `messages` is capped by the core, so only the
        // difference earns a summary and a pass that fitted says nothing extra. The phone groups
        // an account's notifications behind one another, so without this the messages past the
        // cap are not merely unnamed, they are unmentioned.
        let hidden = Int(account.newCount) - account.messages.count
        if hidden > 0 {
            notices.append(
                NewMailNotice(
                    // Its own copy rather than the unknown-sender line: a single hidden message
                    // must not read as one more subject-less message.
                    title: L10n.notification_more_messages(count: hidden),
                    subtitle: account.accountLabel,
                    // The summary stands for messages it does not name, so it has no one body to
                    // quote and quotes none.
                    body: "",
                    identifier: "new-mail-account-\(account.accountId)",
                    threadIdentifier: account.accountId,
                    badge: passTotal,
                    // Nothing to open: a tap brings the app forward and leaves the reader alone.
                    target: nil
                ))
        }
        return notices
    }

    /// What the message is about and how it begins. The account label already holds the subtitle,
    /// so those two share the body, on their own lines, the subject first: a banner truncates from
    /// the end, so the line that must survive goes at the top. The snippet is left out entirely
    /// where the account has none yet, so a notification never ends on a blank line.
    private static func body(_ message: NewMailPreview) -> String {
        let snippet = message.preview.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !snippet.isEmpty else { return message.subject }
        return "\(message.subject)\n\(snippet)"
    }
    #endif

    /// The display name the header carried, else the bare address, else a line of its own: a
    /// notification is never blank, the same rule the avatar's monogram follows.
    private static func sender(_ message: NewMailPreview) -> String {
        if let name = message.senderName?.trimmingCharacters(in: .whitespaces), !name.isEmpty {
            return name
        }
        let address = message.sender.trimmingCharacters(in: .whitespaces)
        return address.isEmpty ? L10n.notification_unknown_sender() : address
    }
}
