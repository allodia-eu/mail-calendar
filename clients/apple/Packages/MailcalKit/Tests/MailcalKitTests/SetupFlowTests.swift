// The last setup step (docs/onboarding.md, "After the connect"): which accounts a new one is
// offered to link to, that a suggestion starts picked and survives only until the person changes
// a pick, that "Add another account" comes back to the step it left, and that each account the
// flow added is asked its name, one after another, once setup closes.

import MailcalBindings
import Testing

@testable import MailcalUI

private func linked(_ id: String) -> LinkedAccount { LinkedAccount(id: id, address: id) }

private func snapshot(_ kind: AccountKind = .imap, suggested: Bool = true) -> AccountsSnapshot {
    AccountsSnapshot(accounts: [
        AccountEntry(
            id: "alice@imap",
            address: "alice@example.org",
            kind: kind,
            uses: [AccountUse(capability: .mail, state: .on)],
            links: AccountLinksView(calendar: nil, contacts: nil, mail: nil),
            linkedFrom: [],
            linkCandidates: LinkCandidates(
                calendar: [linked("bob@dav"), linked("alice@dav")], contacts: [], mail: [],
                suggested: suggested ? ["alice@dav"] : []),
            endpoints: nil)
    ])
}

private func standards(_ kind: AccountKind) -> Bool { kind == .imap || kind == .dav }

private func steps(_ snapshot: AccountsSnapshot) -> (String) -> SetupLinkStep? {
    { SetupLinkStep.step(snapshot, account: $0, offersLinks: standards) }
}

@Suite struct SetupFlowTests {
    @Test func aSuggestionStartsPickedAndContinuingLinksIt() throws {
        let step = try #require(SetupLinkStep.step(snapshot(), account: "alice@imap", offersLinks: standards))
        #expect(step.pickers.count == 1)
        #expect(step.picked == [1])
        #expect(step.links.map(\.slot) == [.calendar])
        #expect(step.links.map(\.target) == ["alice@dav"])
    }

    @Test func nothingToLinkIsNoStep() {
        #expect(SetupLinkStep.step(snapshot(), account: "someone-else", offersLinks: standards) == nil)
        // A provider account is linked from Settings, not offered it at setup.
        #expect(SetupLinkStep.step(snapshot(.google), account: "alice@imap", offersLinks: standards) == nil)
        var lone = snapshot()
        lone.accounts[0].linkCandidates = LinkCandidates(calendar: [], contacts: [], mail: [], suggested: [])
        #expect(SetupLinkStep.step(lone, account: "alice@imap", offersLinks: standards) == nil)
    }

    @Test func theCoreDecidesWhichKindsAreOfferedTheStep() {
        #expect(offersSetupLinks(kind: .imap))
        #expect(offersSetupLinks(kind: .dav))
        #expect(!offersSetupLinks(kind: .google))
    }

    @Test func aPickSurvivesALateSuggestionAndNoneLinksNothing() throws {
        var step = try #require(
            SetupLinkStep.step(snapshot(suggested: false), account: "alice@imap", offersLinks: standards))
        #expect(step.picked == [nil])
        #expect(step.links.isEmpty)

        // Untouched, the step takes the suggestion that arrived.
        let suggested = try #require(steps(snapshot())("alice@imap"))
        step = step.refreshed(suggested)
        #expect(step.picked == [1])

        // Touched, it keeps what the person picked.
        step.pick(0, 0)
        let kept = step.refreshed(suggested)
        #expect(kept.links.map(\.target) == ["bob@dav"])
    }

    @Test func anotherAccountComesBackToTheStepItWasAddedFrom() {
        let stepFor = steps(snapshot())
        var flow = SetupFlow()
        let linking = flow.accountAdded("alice@imap", stepFor: stepFor)
        #expect(linking)
        #expect(flow.links?.account == "alice@imap")

        flow.addLinked()
        #expect(flow.links == nil)
        #expect(flow.returningTo == "alice@imap")

        // The account added from there brings the person back to the step they left, not to one
        // of its own.
        let back = flow.accountAdded("alice@dav", stepFor: stepFor)
        #expect(back)
        #expect(flow.links?.account == "alice@imap")
        let named = flow.finish()
        #expect(named == ["alice@imap", "alice@dav"])
        #expect(flow.links == nil)
        #expect(flow.added.isEmpty)
    }

    @Test func cancellingAnotherAccountReturnsToTheStepAndAFlowWithNothingToLinkEnds() {
        var flow = SetupFlow()
        _ = flow.accountAdded("alice@imap", stepFor: steps(snapshot()))
        flow.addLinked()
        let returned = flow.cancelled(stepFor: steps(snapshot()))
        #expect(returned)
        #expect(flow.links?.account == "alice@imap")

        // Nothing left to return to: the caller ends the flow, and still asks each added name.
        let again = flow.cancelled(stepFor: steps(snapshot()))
        #expect(!again)
        let named = flow.finish()
        #expect(named == ["alice@imap"])

        var lonely = SetupFlow()
        let alone = lonely.accountAdded("someone-else", stepFor: steps(snapshot()))
        #expect(!alone)
    }

    @Test func aLateSuggestionReachesAnUnchangedStep() {
        let unsuggested = steps(snapshot(suggested: false))
        var flow = SetupFlow()
        _ = flow.accountAdded("alice@imap", stepFor: unsuggested)
        flow.refresh(steps(snapshot())("alice@imap"))
        #expect(flow.links?.picked == [1])
    }
}

@Suite @MainActor struct SetupNameQueueTests {
    private func row(_ id: String) -> AccountRow {
        AccountRow(id: id, email: "\(id)@example.test", name: "", expanded: true)
    }

    @Test func eachAddedAccountIsAskedItsNameOneAfterAnother() {
        let model = MailboxModel()
        model.accountWasAdded(row("acct-1"))
        model.accountWasAdded(row("acct-2"))
        // The second waits behind the first rather than replacing its question.
        #expect(model.senderNamePrompt?.id == "acct-1")

        model.senderNamePrompt = nil
        model.senderNameAnswered()
        #expect(model.senderNamePrompt?.id == "acct-2")
    }

    @Test func cancellingWithNothingAddedLeavesSetup() {
        let model = MailboxModel()
        model.addingAccount = true
        model.setupError = "an earlier attempt failed"
        model.cancelSetup()
        #expect(!model.addingAccount)
        #expect(model.setupError == nil)
        #expect(model.senderNamePrompt == nil)
    }
}
