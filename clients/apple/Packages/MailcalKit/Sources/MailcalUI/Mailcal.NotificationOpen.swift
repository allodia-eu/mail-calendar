// Clicking a new-mail notification opens that message (docs/background-sync.md). The click is
// answered by a process-wide delegate (MailNotifier.swift) while the reading pane belongs to a
// scene, so the message waits in a box the shell drains, the same shape the Share Extension's
// drop box has and for the same reason.
//
// Two waits are unavoidable and neither is an error. A tap on a cold app arrives before there is
// a core, and a tap on any app can name a message the list does not hold yet, because the list is
// a window over the mailbox. So the target is kept until a snapshot contains it rather than
// opened against whatever is on screen.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import MailcalBindings
import SwiftUI

/// A clicked notification waiting for the scene, and how far answering it has got.
struct PendingNotificationOpen: Equatable {
    let target: NewMailTarget
    /// Whether the mailbox has already been pointed at the target's account for this click.
    ///
    /// What bounds the retrying. A message can be missing from the list for one snapshot because
    /// the account was only just selected, which is worth waiting for; it can also be missing for
    /// good, because it was deleted or sits outside the loaded window. Without this the second
    /// case would drag the view back to the mailbox on every snapshot the user ever caused,
    /// fighting whatever they navigated to next.
    let navigated: Bool
}

/// Where a message sits in the list: on a row of its own, or inside a conversation.
enum ListedMessage: Equatable {
    case flat(FlatRow)
    case inThread(ThreadRow, ThreadMessage)

    /// The message `rows` hold for `account`/`key`, looking inside every conversation, or `nil`
    /// where the list does not hold it.
    static func find(account: String, key: String, in rows: [SnapshotRow]) -> ListedMessage? {
        guard !account.isEmpty, !key.isEmpty else { return nil }
        for row in rows {
            switch row {
            case .flat(let message) where message.account == account && message.key == key:
                return .flat(message)
            case .thread(let thread):
                if let message = thread.messages.first(where: {
                    $0.account == account && $0.key == key
                }) {
                    return .inThread(thread, message)
                }
            default:
                continue
            }
        }
        return nil
    }
}

/// What the scene does with a clicked notification, given the list it holds.
enum NotificationOpenStep: Equatable {
    /// The core has not opened yet, so there is no list to look in and no account to switch to:
    /// the click waits, unspent. A click that starts the app arrives before the core does.
    case wait
    /// The list holds the message.
    case open(ListedMessage)
    /// The list does not hold it and its account has not been tried for this click: point the
    /// mailbox at that account and try again on the next snapshot.
    case selectAccount(String)
    /// The account was tried and the message is still missing, so the click is spent.
    case drop

    /// The list already on screen comes first, so a message in view opens without moving the
    /// mailbox off the folder the user left it on, and a click that has had its one account
    /// switch is dropped rather than retried for the rest of the session.
    static func next(
        for waiting: PendingNotificationOpen, coreOpen: Bool, rows: [SnapshotRow]
    ) -> NotificationOpenStep {
        guard coreOpen else { return .wait }
        let target = waiting.target
        if let listed = ListedMessage.find(account: target.account, key: target.key, in: rows) {
            return .open(listed)
        }
        return waiting.navigated ? .drop : .selectAccount(target.account)
    }
}

/// Where a clicked notification's message waits for the scene to pick it up.
enum NotificationOpenInbox {
    /// `nonisolated(unsafe)` behind its own lock, not an actor: the delegate that writes is
    /// nonisolated and the scene that reads is main-actor, and a lock is what lets one hand the
    /// other a value without either awaiting the other.
    nonisolated(unsafe) private static var pending: PendingNotificationOpen?
    private static let lock = NSLock()

    /// Posted so an app that is already running drains the box at once rather than at the next
    /// activation. A tap activates the app too, so this is belt and braces; it is what makes the
    /// drain immediate when the app was already frontmost.
    static let arrived = Notification.Name("eu.allodia.mailcal.notificationOpenArrived")

    /// The newest tap wins: an older one names a message the user has moved past by tapping a
    /// second notification, and only one reading pane can answer.
    static func put(_ target: NewMailTarget) {
        lock.lock()
        pending = PendingNotificationOpen(target: target, navigated: false)
        lock.unlock()
        NotificationCenter.default.post(name: arrived, object: nil)
    }

    /// Takes the waiting tap, if any. Taking it spends it.
    static func take() -> PendingNotificationOpen? {
        lock.lock()
        defer { lock.unlock() }
        let waiting = pending
        pending = nil
        return waiting
    }

    /// Puts a taken tap back for a later drain: one waiting for the core, or one navigated for,
    /// so the snapshot the account switch produces gets one more try. Never overwrites a newer
    /// tap.
    static func keep(_ waiting: PendingNotificationOpen) {
        lock.lock()
        if pending == nil {
            pending = waiting
        }
        lock.unlock()
    }
}

/// The moments a clicked notification can reach the reading pane: the tap itself, the activation
/// it causes, the core opening (a tap that started the app lands before it), and the next
/// snapshot, for a message the list had not loaded yet.
struct NotificationOpenRouting: ViewModifier {
    let model: MailboxModel
    let open: (PendingNotificationOpen) -> Void

    #if os(macOS)
    private static let activated = NSApplication.didBecomeActiveNotification
    #else
    private static let activated = UIApplication.didBecomeActiveNotification
    #endif

    func body(content: Content) -> some View {
        content
            .task { drain() }
            .onReceive(NotificationCenter.default.publisher(for: NotificationOpenInbox.arrived)) {
                _ in drain()
            }
            .onReceive(NotificationCenter.default.publisher(for: Self.activated)) { _ in drain() }
            // A tap on a cold app names a message the first snapshot may not carry, and the rows
            // arrive one snapshot at a time. Retrying on each is what turns "not loaded yet" into
            // "opened when it loaded".
            .onChange(of: model.rows.count) { _, _ in drain() }
            .onChange(of: model.app == nil) { _, _ in drain() }
    }

    private func drain() {
        guard let waiting = NotificationOpenInbox.take() else { return }
        open(waiting)
    }
}

extension ContentView {
    /// Opens the message a clicked notification named, once the list holds it
    /// (`NotificationOpenStep`).
    func openNotificationOpen(_ waiting: PendingNotificationOpen) {
        // The reading pane lives in the mail surface, so a click arriving over the calendar or
        // Contacts would open the message behind them and read as a click that did nothing.
        // Not on the retry: that lands a snapshot later, by which time the person may have moved
        // on deliberately.
        let step = NotificationOpenStep.next(
            for: waiting, coreOpen: model.app != nil, rows: model.rows)
        if step == .wait {
            NotificationOpenInbox.keep(waiting)
            return
        }
        if !waiting.navigated { showMail() }
        // The click says where to be, so restoring where the last session was left must not move
        // the mailbox away from it afterwards.
        sceneRestorationComplete = true
        switch step {
        case .open(let listed):
            open(listed)
        case .selectAccount(let account):
            selectAccount(account)
            NotificationOpenInbox.keep(
                PendingNotificationOpen(target: waiting.target, navigated: true))
        case .wait, .drop:
            break
        }
    }
}
