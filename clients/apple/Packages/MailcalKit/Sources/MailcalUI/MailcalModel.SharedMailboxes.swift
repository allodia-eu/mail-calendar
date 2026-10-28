// An account page's traffic with the core about its shared mailboxes (docs/shared-mailboxes.md).
// Listing and looking one up are requests to the server, and switching one off deletes what the
// device holds of it, so each runs off the main actor.

import Foundation
import MailcalBindings

/// What adding a shared mailbox by its address answered.
enum SharedMailboxAdded: Equatable {
    case added
    /// Nothing the account's sign-in opens answers to the address; it stays typed, to correct.
    case notFound
    case failed(String)
}

extension MailboxModel {
    /// Which mailboxes `id`'s sign-in opens besides its own; `nil` for an account that is not set
    /// up, which has no section.
    func sharedMailboxes(_ id: String) async -> SharedMailboxesView? {
        guard let app else { return nil }
        return try? await Task.detached(priority: .userInitiated) {
            try app.sharedMailboxes(accountId: id)
        }.value
    }

    /// Uses `id` for `mailbox`, or stops. Answers why not, or `nil` once stored.
    func setSharedMailbox(_ id: String, _ mailbox: SharedMailboxEntry, on: Bool) async -> String? {
        guard let app else { return L10n.app_unavailable() }
        do {
            try await Task.detached(priority: .userInitiated) {
                try app.setSharedMailbox(accountId: id, mailbox: mailbox, on: on)
            }.value
            return nil
        } catch {
            // Never the core's message: it can name the mailbox, which is someone else's.
            logAppleLifecycle("accounts: switching a shared mailbox failed")
            return coreMessage(error)
        }
    }

    /// Asks the server for the mailbox at `address` and, when `id`'s sign-in opens it, uses the
    /// account for it.
    func addSharedMailbox(_ id: String, address: String) async -> SharedMailboxAdded {
        guard let app else { return .failed(L10n.app_unavailable()) }
        let found: SharedMailboxEntry?
        do {
            found = try await Task.detached(priority: .userInitiated) {
                try app.resolveSharedMailbox(accountId: id, address: address)
            }.value
        } catch {
            // Never the core's message: Microsoft's refusal quotes the address typed, and the
            // mailbox is someone else's.
            logAppleLifecycle("accounts: looking up a shared mailbox failed")
            return .failed(coreMessage(error))
        }
        guard let found else { return .notFound }
        return await setSharedMailbox(id, found, on: true).map(SharedMailboxAdded.failed) ?? .added
    }
}
