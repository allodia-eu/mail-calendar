// What the Shared mailboxes section of an account's page draws for each answer the server gave
// (docs/shared-mailboxes.md rules 3, 7 and 8), decided without a view so each rule is tested.

import Foundation
import MailcalBindings

/// One mailbox's switch, as the rules decide it.
struct SharedMailboxRow: Equatable {
    let title: String
    let subtitle: String?
    let active: Bool
}

enum SharedMailboxRules {
    /// The row for `mailbox`: named by its address where it has one, by the server's label beside
    /// it when that says something else, and marked when the server no longer lists it.
    static func row(_ mailbox: SharedMailboxEntry) -> SharedMailboxRow {
        let title = mailbox.address ?? mailbox.name
        let subtitle: String? = if !mailbox.listed {
            L10n.settings_account_shared_withdrawn()
        } else {
            mailbox.name == title ? nil : mailbox.name
        }
        return SharedMailboxRow(title: title, subtitle: subtitle, active: mailbox.selected)
    }

    /// The line under the switches, when the answer needs one; `nil` when every mailbox the server
    /// listed is a switch above it.
    static func note(_ view: SharedMailboxesView) -> String? {
        switch view.support {
        case .listed: view.mailboxes.isEmpty ? L10n.settings_account_shared_none() : nil
        case .unsupported: nil
        case .byAddress: L10n.settings_account_shared_by_address()
        case .notConnected: L10n.settings_account_shared_not_connected()
        case .unreachable: L10n.settings_account_shared_unreachable()
        }
    }

    /// Whether the page draws the section: once the server has answered, for an account whose
    /// sign-in can open a shared mailbox, so one that cannot has no section that says nothing.
    static func shows(_ view: SharedMailboxesView?) -> Bool {
        guard let view else { return false }
        return view.support != .unsupported || !view.mailboxes.isEmpty
    }

    /// Whether asking again may answer something new without a request to the server: it was not
    /// asked yet, not connected, or could not open a shared mailbox, which a use switched on or a
    /// sign-in completed changes. Listing again otherwise would ask the server on every change.
    static func mayAskAgain(_ view: SharedMailboxesView?) -> Bool {
        guard let view else { return true }
        return view.support == .notConnected || view.support == .unsupported
    }

    /// The address typed into the field, as the server is asked for it; `nil` while there is
    /// nothing to ask.
    static func address(_ typed: String) -> String? {
        let address = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        return address.isEmpty ? nil : address
    }
}
