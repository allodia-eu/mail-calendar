// The servers detection found beside a JMAP server, offered on the link step of the account set up
// from it (docs/onboarding.md, "After the connect: linking another account"): kept for that
// account and that address only, offered only for what its session lacks, and used up once taken.

import MailcalBindings
import Testing

@testable import MailcalUI

private let caldav = "https://dav.fastmail.example/caldav/"
private let carddav = "https://dav.fastmail.example/carddav/"

private func jmapCard(
    email: String = "eva@fastmail.example", caldav: String? = caldav, carddav: String? = carddav,
    nextcloud: Bool = false
) -> DetectedSetup {
    DetectedSetup(
        recommendation: .jmap(
            email: email, serverUrl: "https://api.fastmail.example/", isTrusted: true,
            source: "srv"),
        calendarAndContacts: false, caldavUrl: caldav, carddavUrl: carddav, choices: [],
        nextcloudSignIn: nextcloud)
}

private func jmapAccount(
    calendar: CapabilityState = .notOffered, contacts: CapabilityState = .notOffered
) -> AccountsSnapshot {
    AccountsSnapshot(accounts: [
        AccountEntry(
            id: "eva@jmap", address: "eva@fastmail.example", kind: .jmap, signIn: .password,
            uses: [
                AccountUse(capability: .mail, state: .on),
                AccountUse(capability: .calendar, state: calendar),
                AccountUse(capability: .contacts, state: contacts),
            ],
            links: AccountLinksView(calendar: nil, contacts: nil, mail: nil), linkedFrom: [],
            linkCandidates: LinkCandidates(calendar: [], contacts: [], mail: [], suggested: []),
            endpoints: nil)
    ])
}

@Suite struct SetupBesideTests {
    @Test func onlyAJmapCardThatFoundServersBesideItKeepsThem() {
        #expect(FoundBeside(jmapCard()) != nil)
        #expect(FoundBeside(jmapCard(caldav: nil, carddav: nil)) == nil)
        let dav = DetectedSetup(
            recommendation: .manual(reason: .nothingFound), calendarAndContacts: true,
            caldavUrl: caldav, carddavUrl: carddav, choices: [])
        #expect(FoundBeside(dav) == nil)
    }

    @Test func theOfferIsKeptForTheJmapAccountAddedForThatAddressOnly() {
        var beside = SetupBeside()
        beside.detected(jmapCard())
        beside.added("bob@imap", kind: .imap, addresses: ["eva@fastmail.example"])
        beside.added("bob@jmap", kind: .jmap, addresses: ["bob@fastmail.example"])
        #expect(beside.offers.isEmpty)

        // Added by hand from the card, under the same address in another case, it is kept.
        beside.added("eva@jmap", kind: .jmap, addresses: ["Eva@Fastmail.example"])
        #expect(beside.offer(for: "eva@jmap")?.found.caldavURL == caldav)
        #expect(beside.offer(for: "bob@jmap") == nil)
    }

    @Test func anotherDetectionForgetsWhatTheLastOneFound() {
        var beside = SetupBeside()
        beside.detected(jmapCard())
        beside.detected(jmapCard(caldav: nil, carddav: nil))
        beside.added("eva@jmap", kind: .jmap, addresses: ["eva@fastmail.example"])
        #expect(beside.offers.isEmpty)
    }

    @Test func theOfferIsUsedUpOnceTaken() {
        var beside = SetupBeside()
        beside.detected(jmapCard())
        beside.added("eva@jmap", kind: .jmap, addresses: ["eva@fastmail.example"])
        #expect(beside.take(for: "eva@jmap") != nil)
        #expect(beside.take(for: "eva@jmap") == nil)
        #expect(beside.offer(for: "eva@jmap") == nil)
    }

    @Test func theStepOffersOnlyWhatTheSessionLacksAndSaysWhich() throws {
        var beside = SetupBeside()
        beside.detected(jmapCard())
        beside.added("eva@jmap", kind: .jmap, addresses: ["eva@fastmail.example"])
        let offer = beside.offer(for: "eva@jmap")

        let both = try #require(SetupLinkStep.step(jmapAccount(), account: "eva@jmap", beside: offer))
        #expect(both.beside?.fills == .both)
        #expect(both.beside?.offer.host == "dav.fastmail.example")
        #expect(
            both.beside.map { $0.fills.action($0.offer.host) }
                == L10n.setup_links_beside(host: "dav.fastmail.example"))

        // A session with contacts of its own is offered a calendar, never a second copy.
        let calendar = try #require(
            SetupLinkStep.step(jmapAccount(contacts: .on), account: "eva@jmap", beside: offer))
        #expect(calendar.beside?.fills == .calendar)
        #expect(
            calendar.beside.map { $0.fills.note($0.offer.host) }
                == L10n.setup_links_beside_calendar_note(host: "dav.fastmail.example"))

        // Contacts are looked for at the calendar's server when no address book was found.
        var noBook = SetupBeside()
        noBook.detected(jmapCard(carddav: nil))
        noBook.added("eva@jmap", kind: .jmap, addresses: ["eva@fastmail.example"])
        let contacts = try #require(
            SetupLinkStep.step(
                jmapAccount(calendar: .on), account: "eva@jmap",
                beside: noBook.offer(for: "eva@jmap")))
        #expect(contacts.beside?.fills == .contacts)
    }

    @Test func aServerWithoutACalendarGetsTheStepEvenWithNothingToPick() throws {
        let alone = try #require(SetupLinkStep.step(jmapAccount(), account: "eva@jmap"))
        #expect(alone.pickers.isEmpty)
        #expect(alone.beside == nil)
        // A session with both of its own, and nothing to pick, has no step.
        #expect(
            SetupLinkStep.step(
                jmapAccount(calendar: .on, contacts: .on), account: "eva@jmap",
                beside: SetupBeside().offer(for: "eva@jmap")) == nil)
    }

    @Test func nextcloudIsCarriedToTheCardTheOfferOpens() {
        var beside = SetupBeside()
        beside.detected(jmapCard(nextcloud: true))
        beside.added("eva@jmap", kind: .jmap, addresses: ["eva@fastmail.example"])
        #expect(beside.offer(for: "eva@jmap")?.found.nextcloud == true)
    }
}
