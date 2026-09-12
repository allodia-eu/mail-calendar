// The client-side half of the sender-name step (docs/sending.md): when it opens, and that a
// route which cannot name the account it added opens nothing.
//
// The name itself, its sanitising and its journey to the `From` header are the core's and are
// tested there. What only this layer can get wrong is presenting the step for the wrong account,
// or for none.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite @MainActor struct SenderNameStepTests {

    private func row(_ id: String) -> AccountRow {
        AccountRow(id: id, email: "\(id)@example.test", name: "", expanded: true)
    }

    @Test func addingAnAccountOpensTheStepForThatAccount() {
        let model = MailboxModel()

        model.accountWasAdded(row("acct-2"))

        #expect(model.senderNamePrompt?.id == "acct-2")
    }

    @Test func aRouteThatCannotNameTheAccountOpensNoStep() {
        // Every add route today returns its row, so this is the guard for the next one that
        // does not: asking "your name" without knowing whose would write the name onto
        // whichever account happened to be first.
        let model = MailboxModel()

        model.accountWasAdded()

        #expect(model.senderNamePrompt == nil)
    }

    @Test func addingAnAccountAlsoLeavesTheSetupForm() {
        // The step is a sheet over the running app, so the form underneath has to be gone
        // first, otherwise the account is added and the setup screen is still asking for it.
        let model = MailboxModel()
        model.needsSetup = true
        model.addingAccount = true
        model.setupError = "an earlier attempt failed"

        model.accountWasAdded(row("acct-1"))

        #expect(!model.needsSetup)
        #expect(!model.addingAccount)
        #expect(model.setupError == nil)
    }

    @Test func theStepIsKeyedOnTheAccountSoReopeningTheSameOneIsNotANewSheet() {
        #expect(SenderNamePrompt(id: "acct-1") == SenderNamePrompt(id: "acct-1"))
        #expect(SenderNamePrompt(id: "acct-1").id == "acct-1")
    }
}
