// What a JMAP account is used for, asked after its sign-in (docs/onboarding.md, "After the
// sign-in"): the step opens only when the core offers a choice, starts from what the account is
// used for, holds the last use on, switches on before it switches off, and hands on to the
// separate-server step and the link step the way an add without it does.

import MailcalBindings
import Testing

@testable import MailcalUI

private func choice(_ capability: AccountCapability, on: Bool = true) -> SetupChoice {
    SetupChoice(capability: capability, on: on, serverFound: true)
}

private let everything = [choice(.mail), choice(.calendar), choice(.contacts)]

private func noLinks(_: String) -> SetupLinkStep? { nil }

@Suite struct SetupUsesStepTests {
    @Test func noChoiceIsNoStep() {
        #expect(SetupUsesStep(account: "alice@jmap", choices: []) == nil)
        var flow = SetupFlow()
        let shown1 = flow.accountAdded("alice@jmap", choices: [], stepFor: noLinks)
        #expect(!shown1)
        #expect(flow.uses == nil)
    }

    @Test func eachUseStartsAsTheAccountIsUsedForIt() throws {
        let step = try #require(
            SetupUsesStep(account: "alice@jmap", choices: [choice(.mail), choice(.contacts, on: false)]))
        #expect(step.choices.isOn(.mail))
        #expect(!step.choices.isOn(.contacts))
        // A use the session does not offer is not drawn.
        #expect(step.choices.offer.choice(.calendar) == nil)
        #expect(step.changes.isEmpty)
    }

    @Test func theLastUseIsHeldOn() throws {
        var step = try #require(SetupUsesStep(account: "alice@jmap", choices: everything))
        step.choices.set(.mail, false)
        step.choices.set(.calendar, false)
        #expect(!step.choices.canFlip(.contacts))
        step.choices.set(.contacts, false)
        #expect(step.choices.isOn(.contacts))
    }

    @Test func onIsSwitchedBeforeOffSoTheAccountIsNeverUsedForNothing() throws {
        var step = try #require(SetupUsesStep(
            account: "alice@jmap",
            choices: [choice(.mail), choice(.calendar, on: false), choice(.contacts, on: false)]))
        step.choices.set(.calendar, true)
        step.choices.set(.mail, false)
        #expect(step.changes.map(\.capability) == [.calendar, .mail])
        #expect(step.changes.map(\.on) == [true, false])
    }

    @Test func theStepComesFirstAndHandsOnToTheSeparateServerStep() {
        var flow = SetupFlow()
        let ask = SeparateServerAsk(
            account: "alice@jmap", uses: [.calendar], host: "dav.example.org", username: "alice")
        let shown2 = flow.accountAdded(
            "alice@jmap", choices: everything, separateServer: ask, stepFor: noLinks)
        #expect(shown2)
        #expect(flow.uses?.account == "alice@jmap")
        #expect(flow.separateServer == nil)
        #expect(flow.active)

        let shown3 = flow.usesDone(separateServer: ask, stepFor: noLinks)
        #expect(shown3)
        #expect(flow.uses == nil)
        #expect(flow.separateServer == ask)
    }

    @Test func theStepHandsOnToTheLinkStepItWasAddedFrom() {
        let snapshot = AccountsSnapshot(accounts: [
            AccountEntry(
                id: "bob@imap", address: "bob@example.org", kind: .imap, signIn: .password,
                uses: [AccountUse(capability: .mail, state: .on)],
                links: AccountLinksView(calendar: nil, contacts: nil, mail: nil),
                linkedFrom: [],
                linkCandidates: LinkCandidates(
                    calendar: [LinkedAccount(id: "alice@jmap", address: "alice@example.org")],
                    contacts: [], mail: [], suggested: []))
        ])
        let stepFor = { SetupLinkStep.step(snapshot, account: $0, offersLinks: { _ in true }) }
        var flow = SetupFlow()
        // "Add another account" from bob's link step, then a JMAP account added on the way.
        let shown4 = flow.accountAdded("bob@imap", stepFor: stepFor)
        #expect(shown4)
        flow.addLinked()
        let shown5 = flow.accountAdded("alice@jmap", choices: everything, stepFor: stepFor)
        #expect(shown5)
        #expect(flow.links == nil)
        let shown6 = flow.usesDone(stepFor: stepFor)
        #expect(shown6)
        #expect(flow.links?.account == "bob@imap")
        #expect(flow.finish() == ["bob@imap", "alice@jmap"])
    }

    @Test func aToggleOnScreenIsKept() throws {
        var flow = SetupFlow()
        _ = flow.accountAdded("alice@jmap", choices: everything, stepFor: noLinks)
        var choices = try #require(flow.uses).choices
        choices.set(.calendar, false)
        flow.choose(choices)
        #expect(flow.uses?.changes.map(\.capability) == [.calendar])
    }
}
