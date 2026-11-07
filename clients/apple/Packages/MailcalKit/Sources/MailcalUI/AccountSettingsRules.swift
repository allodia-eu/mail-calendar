// What Settings → Accounts' list says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13), so each rule is tested without a view, and the catalog's words
// for what the core decides on an account's page (`account_page`), which carries no rule here.

import MailcalBindings

enum AccountSettingsRules {
    private static let primary: [AccountCapability] = [.mail, .calendar, .contacts]

    /// The uses the account is used for, working or waiting on a permission, in the snapshot's
    /// order.
    static func inUse(_ entry: AccountEntry) -> [AccountCapability] {
        entry.uses.filter { works($0.state) }.map(\.capability)
    }

    /// Whether the account is used for a use in `state`: on, or waiting on the provider's
    /// permission. A use the server does not offer is not one, whatever was chosen.
    private static func works(_ state: CapabilityState) -> Bool {
        state == .on || state == .needsPermission
    }

    /// The links the account holds, in the order the list row names them.
    static func linked(
        _ entry: AccountEntry
    ) -> [(use: AccountCapability, account: LinkedAccount)] {
        [
            (AccountCapability.calendar, entry.links.calendar),
            (.contacts, entry.links.contacts),
            (.mail, entry.links.mail),
        ].compactMap { use, link in link.map { (use, $0) } }
    }

    /// Whether a use the account is used for waits on the provider's permission.
    static func needsPermission(_ entry: AccountEntry) -> Bool {
        entry.uses.contains { $0.state == .needsPermission }
    }

    /// The uses waiting on the provider's permission, for what a sign-in that withheld some of
    /// them ends with.
    static func withheld(_ entry: AccountEntry) -> [AccountCapability] {
        entry.uses.filter { $0.state == .needsPermission }.map(\.capability)
    }

    /// Whether the account signs in at its provider's own page, which "Sign in again" opens.
    static func signsInAtProvider(_ kind: AccountKind) -> Bool {
        kind == .microsoft || kind == .google
    }

    /// The accounts that lose their link when this one is removed, joined for the confirmation;
    /// `nil` when none does.
    static func unlinked(_ entry: AccountEntry) -> String? {
        entry.linkedFrom.isEmpty
            ? nil : entry.linkedFrom.map { AccountSettingsLabels.name($0) }.joined(separator: ", ")
    }

    /// Whether the mail settings offer a new password or API token: a JMAP account whose pasted
    /// secret the server stopped accepting. An account whose servers are editable takes its new
    /// password there, and one that signs in at a provider signs in again instead.
    static func replacesSecret(
        _ entry: AccountEntry, expired: Bool, provider: AccountProvider?
    ) -> Bool {
        expired && entry.endpoints == nil && provider == .jmap
    }
}

/// The catalog's words for the decisions above.
enum AccountSettingsLabels {
    /// An account's kind, as the setup screen names the same choice.
    static func kind(_ kind: AccountKind) -> String {
        switch kind {
        case .imap: L10n.setup_account_type_password()
        case .dav: L10n.account_kind_dav()
        case .jmap: L10n.setup_account_type_jmap()
        case .microsoft: L10n.setup_account_type_microsoft()
        case .google: L10n.setup_account_type_google()
        }
    }

    /// What a person reads an account by: its address, with what tells it apart from another
    /// account with the same address (docs/accounts.md rule 8).
    static func name(address: String, distinction: AccountDistinction?) -> String {
        switch distinction {
        case nil: address
        case let .kind(accountKind):
            L10n.settings_account_distinguished(address: address, detail: kind(accountKind))
        case let .server(host):
            L10n.settings_account_distinguished(address: address, detail: host)
        }
    }

    static func name(_ account: LinkedAccount) -> String {
        name(address: account.address, distinction: account.distinction)
    }

    static func name(_ entry: AccountEntry) -> String {
        name(address: entry.address, distinction: entry.distinction)
    }

    /// `entry`'s name, or `address` alone for an account the accounts snapshot does not hold.
    static func name(_ entry: AccountEntry?, or address: String) -> String {
        entry.map { name($0) } ?? address
    }

    /// The account's kind, as its row and its page say it under the address. Where another account
    /// shares its address and its kind, the server's host follows: the kind alone would not tell
    /// the two apart.
    static func kindLine(_ entry: AccountEntry) -> String {
        guard case let .server(host) = entry.distinction else { return kind(entry.kind) }
        return "\(kind(entry.kind)) · \(host)"
    }

    /// A use's name, as the main window's sections name it.
    static func use(_ capability: AccountCapability) -> String {
        switch capability {
        case .mail: L10n.nav_mail()
        case .calendar: L10n.nav_calendar()
        case .contacts: L10n.nav_contacts()
        case .colleagues: L10n.account_use_colleagues()
        case .sharedMailboxes: L10n.account_use_shared_mailboxes()
        case .meetings: L10n.account_use_meetings()
        }
    }

    /// The list row's lines under the address: the kind and what the account is used for, what it
    /// is linked to, and whether something needs the person.
    static func summary(_ entry: AccountEntry) -> [String] {
        let uses = AccountSettingsRules.inUse(entry).map(use).joined(separator: ", ")
        var lines = ["\(kindLine(entry)) · \(uses)"]
        lines += AccountSettingsRules.linked(entry).map {
            L10n.settings_account_linked_line(capability: use($0.use), address: name($0.account))
        }
        if AccountSettingsRules.needsPermission(entry) {
            lines.append(L10n.settings_account_needs_permission())
        }
        return lines
    }

    /// The line under a use's switch: what is wrong with the use, then why it cannot be pressed,
    /// so neither hides the other. `nil` when there is nothing to say.
    static func note(_ toggle: AccountUseSwitch) -> String? {
        let status: String? = switch toggle.status {
        case let .problem(problem): problemNote(problem, calendar: toggle.capability == .calendar)
        case .withheld: L10n.settings_account_use_withheld()
        case nil: nil
        }
        let blocked: String? = switch toggle.blocked {
        case .lastUse: L10n.settings_account_use_last()
        case .needsContacts: L10n.settings_account_colleagues_needs_contacts()
        case .needsMail: L10n.settings_account_shared_needs_mail()
        case .notOffered: L10n.settings_account_use_not_offered()
        case .needsServer: L10n.settings_account_use_needs_endpoint()
        case nil: nil
        }
        let lines = [status, blocked].compactMap { $0 }
        return lines.isEmpty ? nil : lines.joined(separator: "\n")
    }

    /// What a change on an account's page came to, when the core says it needs saying.
    static func changeFailed(_ notice: AccountChangeNotice) -> AccountPageAlert {
        switch notice {
        case let .useFailed(_, _, detail), let .linkFailed(_, detail):
            AccountPageAlert(title: L10n.settings_account_change_failed_title(), detail: detail)
        case let .removeFailed(detail):
            AccountPageAlert(title: L10n.remove_account_failed(error: detail), detail: "")
        }
    }

    /// The line under a calendar or an address book that did not connect, which its outage line
    /// above the week or the contacts list repeats.
    static func problemNote(_ problem: UseProblem, calendar: Bool) -> String {
        switch problem {
        case .unreachable:
            calendar
                ? L10n.settings_account_use_unreachable_calendar()
                : L10n.settings_account_use_unreachable_contacts()
        case .signInRefused:
            calendar
                ? L10n.settings_account_use_refused_calendar()
                : L10n.settings_account_use_refused_contacts()
        case .needsOwnSignIn:
            calendar
                ? L10n.settings_account_use_own_sign_in_calendar()
                : L10n.settings_account_use_own_sign_in_contacts()
        case .serverChanged:
            calendar
                ? L10n.settings_account_use_server_changed_calendar()
                : L10n.settings_account_use_server_changed_contacts()
        case .noneOnServer:
            calendar
                ? L10n.settings_account_use_none_on_server_calendar()
                : L10n.settings_account_use_none_on_server_contacts()
        }
    }

    /// A link picker's heading.
    static func slot(_ slot: LinkSlot) -> String {
        switch slot {
        case .calendar: L10n.settings_account_link_calendar()
        case .contacts: L10n.settings_account_link_contacts()
        case .mail: L10n.settings_account_link_mail()
        }
    }

    /// What removing the account does, naming the accounts that lose their link to it.
    static func removal(_ entry: AccountEntry) -> String {
        removal(address: name(entry), unlinked: AccountSettingsRules.unlinked(entry))
    }

    static func removal(address: String, unlinked: String?) -> String {
        let message = L10n.remove_account_message(email: address)
        guard let unlinked else { return message }
        return "\(message) \(L10n.remove_account_unlinks(accounts: unlinked))"
    }

    /// The sign-in section's line: what is wrong, when something is.
    static func signIn(_ entry: AccountEntry, expired: Bool) -> String {
        if expired { return L10n.signin_expired_prompt(accounts: name(entry)) }
        if AccountSettingsRules.needsPermission(entry) {
            return L10n.settings_account_needs_permission()
        }
        return L10n.settings_account_signin_description()
    }
}
