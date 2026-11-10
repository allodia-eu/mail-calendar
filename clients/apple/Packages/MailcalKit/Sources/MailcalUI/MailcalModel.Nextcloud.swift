// Signing in with Nextcloud on the model (docs/nextcloud-sign-in.md): whether a server offers it,
// and setup's completion of a grant. The browser half is NextcloudFlight.swift's. An existing
// account's sign-in with Nextcloud, and a mailbox's separate server's, are the core's
// (MailcalModel.BrowserSignIn.swift). Every core call blocks, so each runs off the main actor; a
// grant is never logged.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Whether the calendar or address-book server at `server` is Nextcloud. Never fails: a
    /// server that cannot be asked offers the password.
    func offersNextcloudSignIn(_ server: String) async -> Bool {
        guard let app else { return false }
        return await Task.detached(priority: .userInitiated) {
            app.offersNextcloudSignIn(serverUrl: server)
        }.value
    }

    // MARK: Setup

    /// Setup's sign-in at the server `server` names, adding a calendar and contacts account used
    /// for `uses`: the found card's and the manual form's.
    func signInWithNextcloud(server: String, uses: [AccountCapability]) {
        setupError = nil
        setupExistingAccount = nil
        setupNextcloud.start(
            app, key: server,
            begin: { try $0.beginNextcloudSignIn(serverUrl: server) },
            complete: { await self.addNextcloudAccount($0, uses: uses) })
    }

    /// Adds the account `grant` signs in to. An account already set up for this person is offered
    /// the grant instead, beneath the refusal.
    private func addNextcloudAccount(
        _ grant: NextcloudGrant, uses: [AccountCapability]
    ) async -> NextcloudCompleted {
        guard let app else {
            setupError = L10n.app_unavailable()
            return .failed(said: nil)
        }
        isConnecting = true
        defer { isConnecting = false }
        do {
            let added = try await Task.detached(priority: .userInitiated) {
                try app.addNextcloudAccount(grant: grant.value, uses: uses)
            }.value
            accountWasAdded(added)
            return .done
        } catch {
            logBrowserSignIn("nextcloud", "the account was not added (\(coreMessage(error)))")
            showSetupFailure(error)
            setupExistingAccount = ExistingAccountUpdate(nextcloudRefusal: error)
            return .failed(said: nil)
        }
    }

    /// Puts setup's grant on the account it was refused as already being, and ends setup as an
    /// add does.
    func updateExistingNextcloudAccount(_ offer: ExistingAccountUpdate) async {
        guard let app else { return }
        updatingExistingAccount = true
        defer { updatingExistingAccount = false }
        let kept = await setupNextcloud.completeHeld(app) { grant in
            do {
                try await Task.detached(priority: .userInitiated) {
                    try app.completeAccountNextcloudSignIn(
                        accountId: offer.accountId, grant: grant.value)
                }.value
                self.setupExistingAccount = nil
                self.accountWasAdded()
                return .done
            } catch {
                logBrowserSignIn("nextcloud", "the account already set up was not signed in (\(coreMessage(error)))")
                self.setupExistingAccount = nil
                self.setupError = setupFailureMessage(error)
                return .failed(said: nil)
            }
        }
        if !kept { setupExistingAccount = nil }
    }

    /// Setup is left: its attempt retires and a grant it kept is withdrawn.
    func leaveSetupNextcloud() {
        setupNextcloud.leave(app)
    }
}

extension ExistingAccountUpdate {
    /// The offer a refused Nextcloud add makes: the grant setup keeps goes on that account.
    init?(nextcloudRefusal error: Error) {
        guard case let MailcalError.AlreadySetUp(accountId) = error else { return nil }
        self.init(accountId: accountId, configToml: "", unreached: [], nextcloud: true)
    }
}
