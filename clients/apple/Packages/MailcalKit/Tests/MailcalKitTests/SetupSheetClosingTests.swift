// The setup sheet keeps drawing the step setup ended on while it goes away. Every path that ends
// on a step clears that step before setup closes, so without the copy the sheet falls through to
// the address screen for the frames its dismissal takes.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite @MainActor struct SetupSheetClosingTests {
    private func linkStep(_ account: String) -> SetupLinkStep? {
        let snapshot = AccountsSnapshot(accounts: [
            AccountEntry(
                id: "alice@imap", address: "alice@example.org", kind: .imap, signIn: .password,
                uses: [AccountUse(capability: .mail, state: .on)],
                links: AccountLinksView(calendar: nil, contacts: nil, mail: nil), linkedFrom: [],
                linkCandidates: LinkCandidates(
                    calendar: [LinkedAccount(id: "alice@dav", address: "alice@dav")], contacts: [],
                    mail: [], suggested: []))
        ])
        return SetupLinkStep.step(snapshot, account: account, offersLinks: { $0 == .imap || $0 == .dav })
    }

    @Test func skippingTheLinkStepKeepsItDrawnUntilSetupIsNextShown() async {
        let model = MailboxModel()
        model.addingAccount = true
        _ = model.setupFlow.accountAdded("alice@imap", stepFor: linkStep)

        await model.setupLinksDone(link: false)

        #expect(!model.addingAccount)
        #expect(model.setupFlow.links == nil)
        #expect(model.setupFlowShown.links?.account == "alice@imap")

        // The next add starts from the address.
        model.addingAccount = true
        #expect(model.setupFlowShown.links == nil)
    }

    @Test func theSeparateServerStepEndingSetupKeepsItDrawn() {
        let model = MailboxModel()
        model.addingAccount = true
        _ = model.setupFlow.accountAdded(
            "alice@imap", separateServer: separateAsk([.calendar]), stepFor: { _ in nil })

        // No account can fill anything, so no link step follows and setup ends here.
        model.setupSeparateServerDone()

        #expect(!model.addingAccount)
        #expect(model.setupFlow.separateServer == nil)
        #expect(model.setupFlowShown.separateServer?.account == "alice@imap")

        // As does a first run, when the last account is removed.
        model.needsSetup = true
        #expect(model.setupFlowShown.separateServer == nil)
    }

    @Test func theUsesStepEndingSetupKeepsItDrawn() async {
        let model = MailboxModel()
        model.addingAccount = true
        let choices = [AccountCapability.mail, .calendar].map {
            SetupChoice(capability: $0, on: true, serverFound: true)
        }
        _ = model.setupFlow.accountAdded("alice@jmap", choices: choices, stepFor: { _ in nil })

        await model.setupUsesDone()

        #expect(!model.addingAccount)
        #expect(model.setupFlow.uses == nil)
        #expect(model.setupFlowShown.uses?.account == "alice@jmap")
    }

    @Test func aFirstRunHasNoSheetToKeepDrawing() async {
        let model = MailboxModel()
        _ = model.setupFlow.accountAdded("alice@imap", stepFor: linkStep)

        await model.setupLinksDone(link: false)

        #expect(model.setupFlowShown.links == nil)
    }
}
