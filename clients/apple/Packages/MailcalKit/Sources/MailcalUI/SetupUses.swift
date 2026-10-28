// What a setup card offers an account for, and what the person's choice makes of it
// (docs/account-autodetect.md rule 8, docs/accounts.md rules 2 and 10). Which uses are offered, and
// how each starts, is the core's (`detect_account_setup`, `provider_setup_choices`); this file
// turns the toggles on screen into the uses and servers the account is set up with. Renderer-free,
// so the rules are tested without a view.

import Foundation
import MailcalBindings

/// The choices the core offered, in its order, and the servers detection found beside the route;
/// an empty URL is one it did not find.
struct UseOffer: Equatable {
    var choices: [SetupChoice]
    var caldavURL = ""
    var carddavURL = ""

    static let none = UseOffer(choices: [])

    /// The offer a detection run answered with.
    init(_ setup: DetectedSetup) {
        choices = setup.choices
        caldavURL = setup.caldavUrl ?? ""
        carddavURL = setup.carddavUrl ?? ""
    }

    init(choices: [SetupChoice], caldavURL: String = "", carddavURL: String = "") {
        self.choices = choices
        self.caldavURL = caldavURL
        self.carddavURL = carddavURL
    }

    /// What a Microsoft or Google sign-in offers for `email`, decided by the core as a detected
    /// card's is.
    static func provider(_ kind: AccountKind, email: String) -> UseOffer {
        UseOffer(choices: providerSetupChoices(kind: kind, email: email))
    }

    func choice(_ capability: AccountCapability) -> SetupChoice? {
        choices.first { $0.capability == capability }
    }

    /// The server shown under a use detection found one for: the address book's own, or the
    /// calendar's, where contacts are looked for when there is none.
    func found(_ capability: AccountCapability) -> String {
        switch capability {
        case .calendar: caldavURL
        case .contacts: carddavURL.isEmpty ? caldavURL : carddavURL
        case .mail, .colleagues, .sharedMailboxes: ""
        }
    }
}

/// A calendar or contacts toggle as the person left it, and the server typed beneath it when
/// detection found none.
struct PickedUse: Equatable {
    var on: Bool
    var typed = ""
}

/// What the person chose, as the account is set up with it.
struct ChosenUses: Equatable {
    /// `nil` when the card offered no choice, so the servers given decide.
    var uses: [AccountCapability]?
    var caldavURL: String
    var carddavURL: String
}

/// Which typed server a choice still needs before the account can be set up.
enum MissingServer: Equatable {
    case none, calendar, contacts
}

/// The toggles a card draws, as one value: what is on, and what was typed beneath each.
struct UseChoices: Equatable {
    let offer: UseOffer
    var mail: Bool?
    var calendar: PickedUse?
    var contacts: PickedUse?
    var colleagues: Bool?

    /// Each use as the core said it starts.
    init(_ offer: UseOffer) {
        self.offer = offer
        mail = offer.choice(.mail)?.on
        calendar = offer.choice(.calendar).map { PickedUse(on: $0.on) }
        contacts = offer.choice(.contacts).map { PickedUse(on: $0.on) }
        colleagues = offer.choice(.colleagues)?.on
    }

    func isOn(_ capability: AccountCapability) -> Bool {
        switch capability {
        case .mail: mail ?? false
        case .calendar: calendar?.on ?? false
        case .contacts: contacts?.on ?? false
        case .colleagues: colleagues ?? false
        // Not offered: this client has no section to choose one (docs/shared-mailboxes.md §3).
        case .sharedMailboxes: false
        }
    }

    /// Whether the person may flip `capability` now. The last use switched on cannot be switched
    /// off, since an account used for nothing is not one; colleagues come from the organisation's
    /// directory beside the person's own contacts, so they wait for contacts.
    func canFlip(_ capability: AccountCapability) -> Bool {
        if capability == .colleagues { return isOn(.contacts) }
        guard isOn(capability) else { return true }
        return [AccountCapability.mail, .calendar, .contacts].filter(isOn).count > 1
    }

    mutating func set(_ capability: AccountCapability, _ on: Bool) {
        guard on == isOn(capability) || canFlip(capability) else { return }
        switch capability {
        case .mail: mail = on
        case .calendar: calendar?.on = on
        case .contacts:
            contacts?.on = on
            if !on, colleagues != nil { colleagues = false }
        case .colleagues: colleagues = on
        case .sharedMailboxes: break
        }
    }

    mutating func type(_ capability: AccountCapability, _ text: String) {
        switch capability {
        case .calendar: calendar?.typed = text
        case .contacts: contacts?.typed = text
        case .mail, .colleagues, .sharedMailboxes: break
        }
    }

    func typed(_ capability: AccountCapability) -> String {
        switch capability {
        case .calendar: calendar?.typed ?? ""
        case .contacts: contacts?.typed ?? ""
        case .mail, .colleagues, .sharedMailboxes: ""
        }
    }

    var chosen: ChosenUses {
        SetupUses.chosen(offer, mail: mail, calendar: calendar, contacts: contacts, colleagues: colleagues)
    }

    var missing: MissingServer { SetupUses.missing(offer, calendar: calendar, contacts: contacts) }
}

enum SetupUses {
    /// The account a card's toggles describe. A calendar stores the endpoint detection found, or
    /// the one typed in its place. Contacts store an address book found apart from the calendar,
    /// or the one typed. Contacts found only through the calendar's server keep that server even
    /// with the calendar off, because that is where they are looked for, and the stored uses keep
    /// the calendar closed. Colleagues count only beside contacts.
    static func chosen(
        _ offer: UseOffer, mail: Bool?, calendar: PickedUse?, contacts: PickedUse?,
        colleagues: Bool?
    ) -> ChosenUses {
        let calendarOn = calendar?.on ?? false
        let contactsOn = contacts?.on ?? false
        func foundOrTyped(_ found: String, _ picked: PickedUse?) -> String {
            found.isEmpty ? (picked?.typed.trimmingCharacters(in: .whitespaces) ?? "") : found
        }
        let carddav = contactsOn && (!offer.carddavURL.isEmpty || offer.caldavURL.isEmpty)
            ? foundOrTyped(offer.carddavURL, contacts) : ""
        let throughCalendar = contactsOn && carddav.isEmpty && !offer.caldavURL.isEmpty
        let caldav = calendarOn || throughCalendar ? foundOrTyped(offer.caldavURL, calendar) : ""
        var uses: [AccountCapability]?
        if !offer.choices.isEmpty {
            var chosen: [AccountCapability] = []
            if mail == true { chosen.append(.mail) }
            if calendarOn { chosen.append(.calendar) }
            if contactsOn {
                chosen.append(.contacts)
                if colleagues == true { chosen.append(.colleagues) }
            }
            uses = chosen
        }
        return ChosenUses(uses: uses, caldavURL: caldav, carddavURL: carddav)
    }

    /// The server still to be typed for a use switched on: the calendar's, or the address book's
    /// when no calendar gives contacts a server to be looked for at.
    static func missing(
        _ offer: UseOffer, calendar: PickedUse?, contacts: PickedUse?
    ) -> MissingServer {
        func empty(_ choice: SetupChoice?, _ picked: PickedUse?) -> Bool {
            choice?.serverFound == false && picked?.on == true
                && picked?.typed.trimmingCharacters(in: .whitespaces).isEmpty == true
        }
        if empty(offer.choice(.calendar), calendar) { return .calendar }
        return calendar?.on != true && empty(offer.choice(.contacts), contacts) ? .contacts : .none
    }

    /// What a manual form's provider sign-in asks for: the uses chosen on screen, less any the
    /// address it signs in with is not offered, so a personal address is never asked for
    /// colleagues.
    static func allowed(
        _ chosen: [AccountCapability]?, _ offered: [SetupChoice]
    ) -> [AccountCapability]? {
        chosen?.filter { capability in offered.contains { $0.capability == capability } }
    }

    /// What the servers typed on the manual calendar-and-contacts form make the account used for:
    /// its calendar beside a calendar server, and contacts beside either, since they are looked
    /// for at the calendar's server when no address book is given. `nil` when neither is typed,
    /// which is no account.
    static func davUses(_ caldav: String, _ carddav: String) -> [AccountCapability]? {
        let hasCalendar = !caldav.trimmingCharacters(in: .whitespaces).isEmpty
        let hasBook = !carddav.trimmingCharacters(in: .whitespaces).isEmpty
        if hasCalendar { return [.calendar, .contacts] }
        return hasBook ? [.contacts] : nil
    }
}
