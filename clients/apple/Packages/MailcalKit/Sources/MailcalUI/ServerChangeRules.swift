// The dialog that asks whether to follow a calendar or address book to a server on another site
// (docs/account-autodetect.md rule 14). The words are the catalog's; which of them, here.

import Foundation
import MailcalBindings

enum ServerChangeLabels {
    private enum Moved {
        case calendar
        case contacts
        case both
    }

    private static func moved(_ change: ServerChange) -> Moved {
        if change.calendar, change.contacts { return .both }
        return change.contacts ? .contacts : .calendar
    }

    static func title(_ change: ServerChange) -> String {
        switch moved(change) {
        case .calendar: L10n.server_changed_title_calendar()
        case .contacts: L10n.server_changed_title_contacts()
        case .both: L10n.server_changed_title_both()
        }
    }

    static func body(_ change: ServerChange, address: String) -> String {
        switch moved(change) {
        case .calendar: L10n.server_changed_body_calendar(address: address)
        case .contacts: L10n.server_changed_body_contacts(address: address)
        case .both: L10n.server_changed_body_both(address: address)
        }
    }

    static func unsure(_ change: ServerChange) -> String {
        switch moved(change) {
        case .calendar: L10n.server_changed_unsure_calendar()
        case .contacts: L10n.server_changed_unsure_contacts()
        case .both: L10n.server_changed_unsure_both()
        }
    }

    /// "Before: {site}", the site in bold.
    static func before(_ change: ServerChange) -> AttributedString {
        emphasised(L10n.server_changed_before(site: change.previousSite), change.previousSite)
    }

    /// "Now: {site}", the site in bold.
    static func now(_ change: ServerChange) -> AttributedString {
        emphasised(L10n.server_changed_now(site: change.proposedSite), change.proposedSite)
    }

    /// `line` as plain text, never markup, with the last occurrence of `site` strongly emphasised:
    /// the site comes from a DNS answer, which is what the person is being asked to judge.
    static func emphasised(_ line: String, _ site: String) -> AttributedString {
        var text = AttributedString(line)
        if !site.isEmpty, let range = text.range(of: site, options: .backwards) {
            text[range].inlinePresentationIntent = .stronglyEmphasized
        }
        return text
    }
}
