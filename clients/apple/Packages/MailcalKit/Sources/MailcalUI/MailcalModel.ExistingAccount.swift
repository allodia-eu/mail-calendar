// An account already set up, refused when setup adds it again (docs/accounts.md rule 17): the
// credential the refused add was given goes on that account rather than being thrown away, after
// the question a credential that would not reach its calendar or contacts asks (rule 18). Every
// call blocks in the core, so each runs off the main actor.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Keeps what the refusal of an add needs to offer updating the account already there.
    /// `unreached` is what the sign-in that produced the config said it would not reach.
    func keepExistingAccountOffer(
        _ error: Error, configToml: String, unreached: [AccountCapability] = []
    ) {
        setupExistingAccount = ExistingAccountUpdate(
            refusal: error, configToml: configToml, unreached: unreached)
    }

    /// Puts the credential the refused add was given on the account already there, and ends
    /// setup as an add does. The account is already in every list, so nothing is added and no
    /// name is asked. A credential that would not reach the account's calendar or contacts asks
    /// through `ask` first; Cancel leaves the refusal on screen.
    func updateExistingAccount(ask: UnreachedAsker? = nil) {
        guard let app, let offer = setupExistingAccount, !updatingExistingAccount else { return }
        if offer.nextcloud {
            Task { await updateExistingNextcloudAccount(offer) }
            return
        }
        Task { @MainActor in
            // Asked before the offer turns to "Connecting", so the button stays behind the alert.
            let unreached: UnreachedUses?
            switch await self.answerUnreached(offer.unreached, ask) {
            case let .proceed(answer): unreached = answer
            case .abandon: return
            }
            guard !self.updatingExistingAccount, self.setupExistingAccount == offer else { return }
            self.updatingExistingAccount = true
            defer { self.updatingExistingAccount = false }
            do {
                try await Task.detached(priority: .userInitiated) {
                    try app.useCredentialFrom(
                        accountId: offer.accountId, configToml: offer.configToml,
                        unreached: unreached)
                }.value
                self.setupExistingAccount = nil
                self.accountWasAdded()
            } catch {
                logAppleLifecycle("setup: updating the account already set up failed (\(coreMessage(error)))")
                self.setupExistingAccount = nil
                self.setupError = setupFailureMessage(error)
            }
        }
    }

    /// What a sign-in hands the core for the uses it said it would not reach, asking the person
    /// through `ask` when there are any.
    func answerUnreached(
        _ unreached: [AccountCapability], _ ask: UnreachedAsker?
    ) async -> UnreachedDecision {
        guard let set = SeparateServerRules.asks(unreached) else { return .proceed(nil) }
        guard let ask else { return SeparateServerRules.decision(unreached, answer: nil) }
        return SeparateServerRules.decision(unreached, answer: await ask(set))
    }
}
