// A calendar or address book the account's sign-in does not reach (docs/accounts.md rule 18): the
// title each set of uses gets, whether a switch asks and what each answer hands the core, and when
// setup shows the separate-server step and where it goes next.

import MailcalBindings
import Testing

@testable import MailcalUI

/// The core's section for a server `uses` need a sign-in of their own at, nothing running.
func separateSection(
    _ uses: [AccountCapability], host: String = "dav.example.org",
    signIn: AccountSeparateSignIn? = nil
) -> AccountSeparateServer {
    AccountSeparateServer(
        uses: uses, host: host, username: "alice", signIn: signIn, connect: .free(),
        stopUsing: .free(), failed: nil)
}

/// The form for `uses` of the mailbox `alice@imap`.
func separateAsk(
    _ uses: [AccountCapability], signIn: AccountSeparateSignIn? = nil
) -> SeparateServerAsk {
    SeparateServerAsk(account: "alice@imap", section: separateSection(uses, signIn: signIn))
}

private func entry(candidates: [LinkedAccount] = []) -> AccountEntry {
    AccountEntry(
        id: "alice@imap",
        address: "alice@example.org",
        kind: .imap,
        signIn: .provider,
        uses: [AccountUse(capability: .mail, state: .on), AccountUse(capability: .calendar, state: .on)],
        links: AccountLinksView(calendar: nil, contacts: nil, mail: nil),
        linkedFrom: [],
        linkCandidates: LinkCandidates(
            calendar: candidates, contacts: [], mail: [], suggested: []))
}

@Suite struct SeparateServerRulesTests {
    @Test func eachSetOfUsesHasItsOwnTitle() {
        #expect(UnreachedSet([.calendar, .contacts]) == .both)
        #expect(UnreachedSet([.calendar]) == .calendar)
        #expect(UnreachedSet([.contacts]) == .contacts)
        #expect(UnreachedSet([]) == nil)
        #expect(UnreachedSet([.mail]) == nil)

        #expect(SeparateServerLabels.unreachedTitle(.both) == L10n.signin_unreached_title_both())
        #expect(
            SeparateServerLabels.unreachedTitle(.calendar) == L10n.signin_unreached_title_calendar())
        #expect(
            SeparateServerLabels.unreachedTitle(.contacts) == L10n.signin_unreached_title_contacts())
        #expect(SeparateServerLabels.formTitle(.both) == L10n.separate_server_title_both())
        #expect(SeparateServerLabels.formTitle(.calendar) == L10n.separate_server_title_calendar())
        #expect(SeparateServerLabels.formTitle(.contacts) == L10n.separate_server_title_contacts())
    }

    @Test func aSwitchAsksOnlyWhenItWouldLoseCalendarOrContacts() {
        #expect(SeparateServerRules.asks([]) == nil)
        #expect(SeparateServerRules.asks([.calendar, .contacts]) == .both)
        // Nothing unreached goes on as before, with no answer.
        #expect(SeparateServerRules.decision([], answer: nil) == .proceed(nil))
        #expect(SeparateServerRules.decision([], answer: .cancel) == .proceed(nil))
    }

    @Test func eachAnswerMapsToWhatTheCoreIsHanded() {
        let unreached: [AccountCapability] = [.calendar]
        #expect(
            SeparateServerRules.decision(unreached, answer: .keep) == .proceed(.keepOnPassword))
        #expect(SeparateServerRules.decision(unreached, answer: .stop) == .proceed(.stopUsing))
        #expect(SeparateServerRules.decision(unreached, answer: .cancel) == .abandon)
        // A question nobody could answer opens no browser.
        #expect(SeparateServerRules.decision(unreached, answer: nil) == .abandon)
    }

    @Test func theFormIsTheSectionTheCoreDrawsForCalendarOrContacts() throws {
        let both = separateSection([.calendar, .contacts], host: "dav.example.org:8443")
        let ask = try #require(SeparateServerRules.ask("alice@imap", both))
        #expect(ask.account == "alice@imap")
        #expect(ask.uses == [.calendar, .contacts])
        #expect(ask.set == .both)
        #expect(ask.host == "dav.example.org:8443")
        #expect(ask.username == "alice")
        #expect(SeparateServerLabels.formBody(ask) == L10n.separate_server_body(host: "dav.example.org:8443"))
        #expect(SeparateServerRules.ask("alice@imap", separateSection([.contacts]))?.set == .contacts)
        #expect(SeparateServerRules.ask("alice@imap", nil) == nil)
        #expect(SeparateServerRules.ask("alice@imap", separateSection([.mail])) == nil)
    }

    @Test func connectWantsALoginAndAPassword() {
        #expect(SeparateServerRules.canConnect("alice", "secret"))
        #expect(!SeparateServerRules.canConnect("alice", ""))
        #expect(!SeparateServerRules.canConnect("  ", "secret"))
    }

    @Test func theRefusalOfAnAccountAlreadySetUpKeepsWhatItsSignInDidNotReach() throws {
        let offer = try #require(
            ExistingAccountUpdate(
                refusal: MailcalError.AlreadySetUp(accountId: "alice@imap"), configToml: "[imap]",
                unreached: [.calendar]))
        #expect(offer.unreached == [.calendar])
        #expect(SeparateServerRules.asks(offer.unreached) == .calendar)
        let plain = try #require(
            ExistingAccountUpdate(
                refusal: MailcalError.AlreadySetUp(accountId: "alice@imap"), configToml: "[imap]"))
        #expect(SeparateServerRules.asks(plain.unreached) == nil)
    }
}

@Suite struct SeparateServerStepTests {
    private let linkable = entry(candidates: [LinkedAccount(id: "alice@dav", address: "alice@dav")])

    private func steps(_ account: AccountEntry) -> (String) -> SetupLinkStep? {
        { id in
            SetupLinkStep.step(
                AccountsSnapshot(accounts: [account]), account: id, offersLinks: { _ in true })
        }
    }

    @Test func setupShowsTheStepOnlyWhenTheAddSaidSo() {
        var plain = SetupFlow()
        let shown = plain.accountAdded("alice@imap", separateServer: nil, stepFor: { _ in nil })
        #expect(!shown)
        #expect(plain.separateServer == nil)
    }

    @Test func theStepComesBeforeTheLinkStepAndHandsOnToIt() {
        let ask = separateAsk([.calendar])
        var flow = SetupFlow()
        let asked = flow.accountAdded("alice@imap", separateServer: ask, stepFor: steps(linkable))
        #expect(asked)
        #expect(flow.separateServer == ask)
        #expect(flow.links == nil)
        #expect(flow.active)

        // Answered, the link step is read afresh for the account.
        let linking = flow.separateServerDone(stepFor: steps(linkable))
        #expect(linking)
        #expect(flow.separateServer == nil)
        #expect(flow.links?.account == "alice@imap")
    }

    @Test func withNothingToLinkTheStepEndsSetup() {
        let ask = separateAsk([.calendar])
        var flow = SetupFlow()
        let asked = flow.accountAdded("alice@imap", separateServer: ask, stepFor: { _ in nil })
        #expect(asked)
        let linking = flow.separateServerDone(stepFor: { _ in nil })
        #expect(!linking)
        #expect(flow.separateServer == nil)
        let named = flow.finish()
        #expect(named == ["alice@imap"])
    }
}
