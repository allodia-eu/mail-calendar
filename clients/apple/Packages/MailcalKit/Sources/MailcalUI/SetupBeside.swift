// The calendar and address-book servers detection found beside a JMAP server, offered on the link
// step of the account set up from it as an account of their own, for what its session lacks and
// only that (docs/onboarding.md, "After the connect: linking another account";
// `MailcalApp::dav_setup_beside`). Renderer-free, so it is tested without a view.

import Foundation
import MailcalBindings

/// The servers detection found beside a JMAP server, for the address it was found for.
struct FoundBeside: Equatable {
    let email: String
    let caldavURL: String?
    let carddavURL: String?
    /// They are Nextcloud, which signs in through the browser.
    let nextcloud: Bool

    /// What a detected JMAP card found beside its server; `nil` for any other card, or one that
    /// found nothing beside it.
    init?(_ setup: DetectedSetup) {
        guard case let .jmap(email, _, _, _) = setup.recommendation,
              setup.caldavUrl != nil || setup.carddavUrl != nil
        else { return nil }
        self.email = email
        caldavURL = setup.caldavUrl
        carddavURL = setup.carddavUrl
        nextcloud = setup.nextcloudSignIn
    }
}

/// The offer for one account: the servers found beside the JMAP server it was set up from.
struct BesideOffer: Equatable {
    let account: String
    let found: FoundBeside

    /// The host a person recognises the servers by.
    var host: String {
        (found.caldavURL ?? found.carddavURL).map(urlHost) ?? ""
    }
}

/// What the servers found beside a JMAP server would fill of what its session lacks.
enum BesideFills: Equatable {
    case calendar
    case contacts
    case both

    /// Contacts are looked for at the calendar's server when no address book was found, as
    /// `dav_setup_beside` does. `nil` when the servers fill nothing the account lacks.
    init?(_ found: FoundBeside, lacksCalendar: Bool, lacksContacts: Bool) {
        let calendar = lacksCalendar && found.caldavURL != nil
        let contacts = lacksContacts && (found.carddavURL != nil || found.caldavURL != nil)
        switch (calendar, contacts) {
        case (true, true): self = .both
        case (true, false): self = .calendar
        case (false, true): self = .contacts
        case (false, false): return nil
        }
    }

    func note(_ host: String) -> String {
        switch self {
        case .calendar: L10n.setup_links_beside_calendar_note(host: host)
        case .contacts: L10n.setup_links_beside_contacts_note(host: host)
        case .both: L10n.setup_links_beside_note(host: host)
        }
    }

    func action(_ host: String) -> String {
        switch self {
        case .calendar: L10n.setup_links_beside_calendar(host: host)
        case .contacts: L10n.setup_links_beside_contacts(host: host)
        case .both: L10n.setup_links_beside(host: host)
        }
    }
}

/// The offer on a link step, and what it fills.
struct BesideStep: Equatable {
    let offer: BesideOffer
    let fills: BesideFills
}

/// What one setup flow knows: the servers found beside the JMAP server on screen, kept past "Set
/// up manually", and the offer kept for each JMAP account added from it until its link step uses
/// it.
struct SetupBeside: Equatable {
    private(set) var found: FoundBeside?
    private(set) var offers: [BesideOffer] = []

    /// Detection answered, or a card was opened: what it found beside a JMAP server, or nothing.
    mutating func detected(_ setup: DetectedSetup) {
        found = FoundBeside(setup)
    }

    /// `account` was added. A JMAP account set up for the address the servers were found for,
    /// from the card or by hand from it, keeps them for its link step.
    mutating func added(_ account: String, kind: AccountKind, addresses: [String]) {
        guard kind == .jmap, let found,
              addresses.contains(where: { $0.caseInsensitiveCompare(found.email) == .orderedSame })
        else { return }
        offers.removeAll { $0.account == account }
        offers.append(BesideOffer(account: account, found: found))
    }

    /// The offer kept for `account`, while unused.
    func offer(for account: String) -> BesideOffer? {
        offers.first { $0.account == account }
    }

    /// The link step's offer was taken: it is used up.
    mutating func take(for account: String) -> BesideOffer? {
        guard let index = offers.firstIndex(where: { $0.account == account }) else { return nil }
        return offers.remove(at: index)
    }

    /// The flow ended.
    mutating func reset() {
        self = SetupBeside()
    }
}
