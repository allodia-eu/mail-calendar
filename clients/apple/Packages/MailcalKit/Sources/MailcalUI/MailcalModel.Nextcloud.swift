// Signing in with Nextcloud on the model (docs/nextcloud-sign-in.md): whether a server offers it,
// and each surface's completion of a grant. The browser half is NextcloudFlight.swift's. Every
// core call blocks, so each runs off the main actor; a grant is never logged.

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

    /// Whether the server holding `ask`'s uses is Nextcloud.
    func separateServerOffersNextcloudSignIn(_ ask: SeparateServerAsk) async -> Bool {
        guard let app else { return false }
        return await Task.detached(priority: .userInitiated) {
            app.separateServerOffersNextcloudSignIn(accountId: ask.account, uses: ask.uses)
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

    // MARK: A mailbox's separate server

    /// Moves `ask`'s uses to an account of their own that `grant` signs in to, linked to the
    /// mailbox. Answers why not, or `nil` once moved.
    func addLinkedNextcloudAccount(_ ask: SeparateServerAsk, _ grant: NextcloudGrant) async
        -> String?
    {
        guard let app else { return L10n.app_unavailable() }
        do {
            _ = try await Task.detached(priority: .userInitiated) {
                try app.addLinkedNextcloudAccount(
                    accountId: ask.account, uses: ask.uses, grant: grant.value)
            }.value
            syncAfterAccountChange()
            refreshAccountsSnapshot()
            return nil
        } catch {
            logBrowserSignIn("nextcloud", "the separate server was not signed in (\(coreMessage(error)))")
            return SeparateServerRules.signInFailure(error)
        }
    }

    // MARK: An account signing in again

    /// Signs the calendar and contacts account `id` in with Nextcloud: again, or in place of its
    /// typed password. `reported` hears a failure, for a surface that says it elsewhere.
    func signInAccountWithNextcloud(_ id: String, reported: ((String) -> Void)? = nil) {
        accountNextcloud.start(
            app, key: id,
            begin: { try $0.beginAccountNextcloudSignIn(accountId: id) },
            complete: { await self.completeAccountNextcloudSignIn(id, $0) },
            reported: reported)
    }

    private func completeAccountNextcloudSignIn(_ id: String, _ grant: NextcloudGrant) async
        -> NextcloudCompleted
    {
        guard let app else { return .failed(said: L10n.app_unavailable()) }
        do {
            try await Task.detached(priority: .userInitiated) {
                try app.completeAccountNextcloudSignIn(accountId: id, grant: grant.value)
            }.value
            refreshAccountsSnapshot()
            return .done
        } catch {
            logBrowserSignIn("nextcloud", "the account was not signed in (\(coreMessage(error)))")
            return .failed(said: coreMessage(error))
        }
    }

    /// The expired-sign-in banner's sign-in, for an account a Nextcloud sign-in made. A success
    /// says nothing, because the banner clearing is the answer.
    func signInWithNextcloudFromBanner(_ id: String) {
        signInAccountWithNextcloud(id) { [weak self] said in self?.accountNotice = said }
    }
}

extension ExistingAccountUpdate {
    /// The offer a refused Nextcloud add makes: the grant setup keeps goes on that account.
    init?(nextcloudRefusal error: Error) {
        guard case let MailcalError.AlreadySetUp(accountId) = error else { return nil }
        self.init(accountId: accountId, configToml: "", unreached: [], nextcloud: true)
    }
}
