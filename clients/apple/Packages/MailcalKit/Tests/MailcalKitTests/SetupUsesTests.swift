// What a setup card's toggles make of an account (docs/account-autodetect.md rule 8,
// docs/accounts.md rules 2 and 10): which uses it is set up with, and which servers it stores.
// The Apple client's copy of the suite Linux (`setup_uses_tests`) and Windows (`SetupUsesTests`)
// carry, and the toggles' own rules.

import MailcalBindings
import Testing

@testable import MailcalUI

private let calDav = "https://dav.example.org/caldav"
private let cardDav = "https://dav.example.org/carddav"
private let on = PickedUse(on: true)
private let off = PickedUse(on: false)

private func offer(_ caldav: String, _ carddav: String) -> UseOffer {
    func choice(_ capability: AccountCapability, _ found: Bool) -> SetupChoice {
        SetupChoice(capability: capability, on: found, serverFound: found)
    }
    return UseOffer(
        choices: [
            choice(.mail, true),
            choice(.calendar, !caldav.isEmpty),
            choice(.contacts, !caldav.isEmpty || !carddav.isEmpty),
        ],
        caldavURL: caldav, carddavURL: carddav)
}

private func provider() -> UseOffer {
    UseOffer(choices: [AccountCapability.mail, .calendar, .contacts, .colleagues].map {
        SetupChoice(capability: $0, on: true, serverFound: true)
    })
}

@Suite struct SetupUsesTests {
    @Test func everythingFoundAndKeptStoresBothEndpointsAndEveryUse() {
        let chosen = SetupUses.chosen(
            offer(calDav, cardDav), mail: true, calendar: on, contacts: on, colleagues: nil)
        #expect(chosen.uses == [.mail, .calendar, .contacts])
        #expect(chosen.caldavURL == calDav)
        #expect(chosen.carddavURL == cardDav)
    }

    @Test func contactsFoundThroughTheCalendarKeepItsServerWithTheCalendarOff() {
        let kept = SetupUses.chosen(
            offer(calDav, ""), mail: true, calendar: off, contacts: on, colleagues: nil)
        #expect(kept.uses == [.mail, .contacts])
        #expect(kept.caldavURL == calDav)
        #expect(kept.carddavURL.isEmpty)
    }

    @Test func aUseSwitchedOffStoresNoServer() {
        let mailOnly = SetupUses.chosen(
            offer(calDav, cardDav), mail: true, calendar: off, contacts: off, colleagues: nil)
        #expect(mailOnly.uses == [.mail])
        #expect((mailOnly.caldavURL + mailOnly.carddavURL).isEmpty)

        let withoutMail = SetupUses.chosen(
            offer(calDav, cardDav), mail: false, calendar: on, contacts: off, colleagues: nil)
        #expect(withoutMail.uses == [.calendar])
        #expect(withoutMail.carddavURL.isEmpty)
    }

    @Test func aServerNotFoundIsTheOneTyped() {
        let typed = SetupUses.chosen(
            offer("", ""), mail: true, calendar: PickedUse(on: true, typed: " cal.example.org "),
            contacts: PickedUse(on: true, typed: "book.example.org"), colleagues: nil)
        #expect(typed.caldavURL == "cal.example.org")
        #expect(typed.carddavURL == "book.example.org")

        // A box typed into and then switched off stores nothing.
        let dropped = SetupUses.chosen(
            offer("", ""), mail: true, calendar: PickedUse(on: false, typed: "cal.example.org"),
            contacts: off, colleagues: nil)
        #expect(dropped.caldavURL.isEmpty)
    }

    @Test func aCardThatOfferedNoChoiceLeavesTheUsesToTheServers() {
        let none = UseOffer(choices: [], caldavURL: calDav)
        #expect(SetupUses.chosen(none, mail: nil, calendar: nil, contacts: nil, colleagues: nil).uses == nil)
    }

    @Test func contactsAreLookedForAtACalendarTypedInPlaceOfOneFound() {
        let typed = SetupUses.chosen(
            offer("", ""), mail: true, calendar: PickedUse(on: true, typed: "cal.example.org"),
            contacts: on, colleagues: nil)
        #expect(typed.caldavURL == "cal.example.org")
        #expect(typed.carddavURL.isEmpty)
        #expect(typed.uses == [.mail, .calendar, .contacts])
    }

    @Test func colleaguesCountOnlyBesideContacts() {
        #expect(
            SetupUses.chosen(provider(), mail: false, calendar: off, contacts: on, colleagues: true).uses
                == [.contacts, .colleagues])
        #expect(
            SetupUses.chosen(provider(), mail: true, calendar: off, contacts: off, colleagues: true).uses
                == [.mail])
        // A provider names no server, and none is stored.
        let everything = SetupUses.chosen(
            provider(), mail: true, calendar: on, contacts: on, colleagues: true)
        #expect((everything.caldavURL + everything.carddavURL).isEmpty)
    }

    @Test func aUseSwitchedOnWithNoServerAsksForOne() {
        let nothingFound = offer("", "")
        #expect(SetupUses.missing(nothingFound, calendar: on, contacts: on) == .calendar)
        #expect(
            SetupUses.missing(nothingFound, calendar: PickedUse(on: true, typed: "cal"), contacts: on)
                == MissingServer.none)
        // Without a calendar, contacts need an address book of their own.
        #expect(SetupUses.missing(nothingFound, calendar: off, contacts: on) == .contacts)
        #expect(SetupUses.missing(nothingFound, calendar: off, contacts: off) == MissingServer.none)
        // A server that was found needs nothing typed.
        #expect(SetupUses.missing(offer(calDav, ""), calendar: on, contacts: on) == MissingServer.none)
    }

    @Test func aManualSignInAsksOnlyForWhatTheAddressIsOffered() {
        let chosen: [AccountCapability] = [.mail, .contacts, .colleagues]
        // A personal address is offered no colleagues (docs/accounts.md rule 3).
        let personal = provider().choices.filter { $0.capability != .colleagues }
        #expect(SetupUses.allowed(chosen, personal) == [.mail, .contacts])
        #expect(SetupUses.allowed(chosen, provider().choices) == chosen)
        #expect(SetupUses.allowed(nil, provider().choices) == nil)
    }

    @Test func theServersTypedDecideWhatACalendarAndContactsAccountIsUsedFor() {
        #expect(SetupUses.davUses("cloud.example", "") == [.calendar, .contacts])
        #expect(SetupUses.davUses("cloud.example", "book.example") == [.calendar, .contacts])
        #expect(SetupUses.davUses("", "book.example") == [.contacts])
        #expect(SetupUses.davUses(" ", "") == nil)
    }

    @Test func eachUseStartsAsTheCoreSaidAndTheLastOneIsHeldOn() {
        var choices = UseChoices(offer(calDav, ""))
        #expect(choices.isOn(.mail) && choices.isOn(.calendar) && choices.isOn(.contacts))
        choices.set(.calendar, false)
        choices.set(.contacts, false)
        #expect(!choices.canFlip(.mail))
        // An account used for nothing is not one: the last use stays on.
        choices.set(.mail, false)
        #expect(choices.isOn(.mail))
        #expect(choices.canFlip(.calendar))
    }

    @Test func colleaguesWaitForContactsAndGoWithThem() {
        var choices = UseChoices(provider())
        #expect(choices.isOn(.colleagues))
        choices.set(.contacts, false)
        #expect(!choices.isOn(.colleagues))
        #expect(!choices.canFlip(.colleagues))
        choices.set(.colleagues, true)
        #expect(!choices.isOn(.colleagues))
    }

    @Test func aDiscoveredEndpointIsShownByHost() {
        #expect(urlHost("https://caldav.example.test/dav/") == "caldav.example.test")
        #expect(urlHost("not a url") == "not a url")
        // Contacts found only through the calendar show the calendar's server.
        #expect(offer(calDav, "").found(.contacts) == calDav)
        #expect(offer(calDav, cardDav).found(.contacts) == cardDav)
    }
}
