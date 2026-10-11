// What follows a connect, for every way an account is added: the link step (docs/onboarding.md,
// "After the connect"), the accounts an "Add another account" adds on the way, the name each of
// them is asked for once setup closes (docs/sending.md), and, last, what a provider's sign-in
// withheld, said on that account's page in Settings (docs/accounts.md rule 10).

import Foundation
import MailcalBindings

/// A line at the top of an account's page in Settings.
struct AccountPageNotice: Equatable {
    let account: String
    let title: String
    let detail: String
}

extension MailboxModel {
    /// Every route that adds an account ends here: the manual form, the Microsoft and Google
    /// sign-ins, and the JMAP and IMAP ones (whose routes converge before this).
    ///
    /// Setup moves on to the link step when another account can fill something this one lacks,
    /// back to the step an "Add another account" left, or closes. The pass to the person's other
    /// devices belongs to every route rather than to the form: an account added without one stays
    /// on this device until the next launch, and its page draws no sharing control.
    ///
    /// `account` is the row the add returned, when the route has it. A route with none ends the
    /// flow without asking a name, rather than guessing which account it just added.
    func accountWasAdded(_ account: AccountRow? = nil) {
        setupError = nil
        syncAfterAccountChange()
        refreshAccountsSnapshot()
        guard let account else {
            finishSetup()
            return
        }
        if let entry = accountEntry(account.id) {
            let withheld = AccountSettingsRules.withheld(entry)
            if !withheld.isEmpty {
                withheldWaiting = AccountPageNotice(
                    account: account.id,
                    title: L10n.setup_withheld_title(),
                    detail: L10n.setup_withheld_detail(
                        uses: withheld.map(AccountSettingsLabels.use).joined(separator: ", ")))
            }
        }
        if !setupFlow.accountAdded(account.id, stepFor: setupLinkStep) {
            finishSetup()
        }
    }

    func setupLinkStep(_ account: String) -> SetupLinkStep? {
        SetupLinkStep.step(accountsSnapshot, account: account)
    }

    /// The link step answered: the picks are linked when `link`, and setup closes either way.
    func setupLinksDone(link: Bool) async {
        guard !savingSetupLinks else { return }
        savingSetupLinks = true
        if link, let step = setupFlow.links {
            for (slot, target) in step.links {
                if let failure = await setAccountLink(step.account, slot, target) {
                    logAppleLifecycle("setup: a link chosen at setup was not stored (\(failure))")
                }
            }
        }
        savingSetupLinks = false
        finishSetup()
    }

    /// "Add another account": setup starts again from the address, and comes back to this step,
    /// cancelled or done.
    func setupAddLinkedAccount() {
        guard !savingSetupLinks else { return }
        setupFlow.addLinked()
        setupStartEmail = ""
        setupStartOffer = nil
        clearSetupAttempt()
    }

    /// Whether setup offers a Cancel: an add over the running app, or a link step an "Add another
    /// account" left, which is where Cancel goes back to.
    var setupCancellable: Bool { addingAccount || setupFlow.returningTo != nil }

    /// Cancel on the setup form: back to the link step "Add another account" left, the end of a
    /// flow that added an account, which ends as a skipped link step does, or out of setup.
    func cancelSetup() {
        clearSetupAttempt()
        if setupFlow.cancelled(stepFor: setupLinkStep) { return }
        if setupFlow.active {
            finishSetup()
            return
        }
        addingAccount = false
        setupStartEmail = ""
        setupStartOffer = nil
    }

    /// What the last attempt on the setup form left behind: the error, and a certificate it was
    /// refused or accepted.
    func clearSetupAttempt() {
        setupError = nil
        setupRejectedCertificate = nil
        setupAcceptedCertificate = nil
    }

    /// Closes setup and asks each added account's name, one after another.
    func finishSetup() {
        senderNamesWaiting += setupFlow.finish()
        needsSetup = false
        addingAccount = false
        setupStartEmail = ""
        setupStartOffer = nil
        if !askingSenderName { askNextSenderName() }
    }

    /// The name step was answered or skipped: the next account waiting is asked.
    func senderNameAnswered() {
        askingSenderName = false
        askNextSenderName()
    }

    /// Asks the next account waiting, where the core says it still needs a name: a provider that
    /// already holds one has had it adopted, and an account without mail sends nothing. Once none
    /// is waiting, what a sign-in's grant withheld is said on that account's page.
    ///
    /// The question is a provider round trip, so it runs off the main actor. With no core to ask
    /// the step opens: skipping it is the answer that cannot be taken back, since it leaves the
    /// account sending as a bare address, and the step itself is one action to dismiss.
    private func askNextSenderName() {
        guard !senderNamesWaiting.isEmpty else {
            if let withheld = withheldWaiting {
                withheldWaiting = nil
                accountPageNotice = withheld
                openAccountPage(withheld.account)
            }
            return
        }
        let next = senderNamesWaiting.removeFirst()
        askingSenderName = true
        guard let app else {
            senderNamePrompt = SenderNamePrompt(id: next)
            return
        }
        Task {
            let needed = await Task.detached(priority: .userInitiated) {
                app.needsSenderName(account: next)
            }.value
            if needed {
                senderNamePrompt = SenderNamePrompt(id: next)
            } else {
                senderNameAnswered()
            }
        }
    }

    /// A suggestion the core found after the link step was drawn reaches it, unless the person
    /// has changed the step already.
    func refreshSetupLinks() {
        guard !savingSetupLinks, let step = setupFlow.links else { return }
        setupFlow.refresh(setupLinkStep(step.account))
    }
}
