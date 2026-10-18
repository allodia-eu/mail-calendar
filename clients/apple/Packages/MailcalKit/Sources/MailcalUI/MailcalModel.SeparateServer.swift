// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): moved to an account of their own with a password typed for that server, or switched off.
// The core adds and connects the calendar account before the mailbox changes, so a refusal leaves
// both as they were. Every call blocks in the core, so each runs off the main actor.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Moves `ask`'s uses to an account of their own at the same server, signing in as
    /// `username` with `password`, and links it to the mailbox. Answers why not, or `nil` once
    /// moved; the page and the link step then redraw from the next snapshot.
    func addLinkedCalendarAccount(
        _ ask: SeparateServerAsk, username: String, password: String
    ) async -> String? {
        guard let app else { return L10n.app_unavailable() }
        do {
            _ = try await Task.detached(priority: .userInitiated) {
                try app.addLinkedCalendarAccount(
                    accountId: ask.account, uses: ask.uses, username: username, password: password)
            }.value
            // An account was added, so it travels as every added account does.
            syncAfterAccountChange()
            refreshAccountsSnapshot()
            return nil
        } catch {
            // Never the login or the password.
            logAppleLifecycle(
                "accounts: the calendar server's own sign-in failed (\(coreMessage(error)))")
            return coreMessage(error)
        }
    }

    /// Skip on the setup step: the uses the sign-in did not reach are switched off, so the account
    /// does not open on a calendar that cannot connect. Its page offers the same form later.
    func skipSeparateServer(_ ask: SeparateServerAsk) async {
        // A failure is logged by the switch itself, and setup goes on: the page still offers it.
        for use in SeparateServerRules.skipped(ask) {
            _ = await setAccountUse(ask.account, use, on: false)
        }
    }
}
