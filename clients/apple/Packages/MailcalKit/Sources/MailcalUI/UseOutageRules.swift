// The line the calendar and contacts screens draw for a calendar or an address book that did not
// connect on an account that did (docs/accounts.md rule 16, docs/calendar.md §4,
// docs/contacts.md §3). The words are the catalog's; the composition is here, so it is tested
// without a view.

import MailcalBindings

/// Which use an outage line speaks for.
enum OutageUse: Equatable {
    case calendar
    case contacts
}

/// One line above the week or the contacts list.
struct UseOutageLine: Equatable, Identifiable {
    /// The account whose page the line's action opens.
    let account: String
    let text: String

    var id: String { account }
}

enum UseOutageRules {
    /// The title, the reason Settings → Accounts gives under that use's switch, and the
    /// last-synced sentence.
    static func text(_ problem: UseProblem, _ use: OutageUse, address: String) -> String {
        let title = switch use {
        case .calendar: L10n.calendar_outage_title(address: address)
        case .contacts: L10n.contacts_outage_title(address: address)
        }
        let reason = AccountSettingsLabels.problemNote(problem, calendar: use == .calendar)
        return [title, reason, L10n.use_outage_last_synced()].joined(separator: " ")
    }

    /// One line per outage, in the snapshot's order. `address` names an account by the address a
    /// person knows it by.
    static func lines(
        _ outages: [UseOutage], _ use: OutageUse, address: (String) -> String
    ) -> [UseOutageLine] {
        outages.map {
            UseOutageLine(
                account: $0.accountId,
                text: text($0.problem, use, address: address($0.accountId))
            )
        }
    }
}

extension MailboxModel {
    /// The lines above the calendar week.
    var calendarOutageLines: [UseOutageLine] {
        UseOutageRules.lines(connectivity?.calendarOutages ?? [], .calendar, address: accountEmail)
    }

    /// The lines above the contacts list.
    var contactsOutageLines: [UseOutageLine] {
        UseOutageRules.lines(connectivity?.contactsOutages ?? [], .contacts, address: accountEmail)
    }
}
