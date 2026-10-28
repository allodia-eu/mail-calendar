// What Settings → Accounts says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13), so each rule is tested without a view. The words are the
// catalog's; the decisions are here.

import MailcalBindings

/// Which line, if any, sits under a use's switch.
enum UseNote: Equatable {
    case none
    /// The last of mail, calendar and contacts: the account is removed instead.
    case last
    /// Colleagues, while contacts are off.
    case needsContacts
    /// Chosen, and the provider has not granted it.
    case withheld
    /// The server offers none, so nothing can switch it on.
    case notOffered
    /// On, and it did not connect although the account did: a calendar or an address book, since
    /// mail decides whether the account connected at all.
    case problem(UseProblem, AccountCapability)
}

/// How one use's switch is drawn.
struct UseSwitch: Equatable {
    /// On. A use waiting on a permission is drawn off: it is not working, and switching it on is
    /// what asks the provider again.
    let active: Bool
    /// Whether switching it on asks the provider rather than the core.
    let asks: Bool
    /// Whether the person can flip it.
    let enabled: Bool
    let note: UseNote
}

/// One link slot's picker: "none" first, then `options`.
struct LinkPicker: Equatable {
    let slot: LinkSlot
    let options: [LinkedAccount]
    /// The index into `options` of the account linked now; `nil` for "none".
    let selected: Int?
    /// The index of the account to suggest while nothing is linked: one whose calendar server
    /// schedules as the mail account's address.
    let suggested: Int?
}

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

    /// The switch for `capability`. The last of mail, calendar and contacts cannot be switched
    /// off, and colleagues cannot be switched on without contacts: the core refuses both, so the
    /// switch says why rather than letting the person find out.
    static func useSwitch(_ entry: AccountEntry, _ capability: AccountCapability) -> UseSwitch {
        func used(_ wanted: AccountCapability) -> Bool {
            entry.uses.contains { $0.capability == wanted && works($0.state) }
        }
        let state = entry.uses.first { $0.capability == capability }?.state ?? .off
        // A use waiting on a permission is drawn off, so it is never the one held on: switching
        // it on asks the provider, which the core does not refuse.
        let last = state == .on && primary.contains(capability)
            && primary.filter(used).count == 1
        let withoutContacts = capability == .colleagues && state == .off && !used(.contacts)
        let notOffered = state == .notOffered
        let problem = entry.uses.first { $0.capability == capability }?.problem
        let note: UseNote
        if notOffered {
            note = .notOffered
        } else if let problem {
            note = .problem(problem, capability)
        } else if last {
            note = .last
        } else if withoutContacts {
            note = .needsContacts
        } else if state == .needsPermission {
            note = .withheld
        } else {
            note = .none
        }
        return UseSwitch(
            active: state == .on,
            asks: state == .needsPermission,
            enabled: !last && !withoutContacts && !notOffered,
            note: note
        )
    }

    /// Whether the line under a use's switch offers "Review": the use is paused for a server on
    /// another site, which only the person can allow.
    static func reviewsServerChange(_ note: UseNote) -> Bool {
        if case .problem(.serverChanged, _) = note { true } else { false }
    }

    /// The pickers an account's page offers: one per slot it can hold, which is a slot with
    /// candidates or one already linked. A link the candidates no longer offer is kept as the
    /// first option, so the picker shows what is stored rather than "none".
    static func linkPickers(_ entry: AccountEntry) -> [LinkPicker] {
        let slots: [(LinkSlot, [LinkedAccount], LinkedAccount?)] = [
            (.calendar, entry.linkCandidates.calendar, entry.links.calendar),
            (.contacts, entry.linkCandidates.contacts, entry.links.contacts),
            (.mail, entry.linkCandidates.mail, entry.links.mail),
        ]
        return slots.compactMap { slot, candidates, current in
            guard !candidates.isEmpty || current != nil else { return nil }
            var options = candidates
            if let current, !options.contains(where: { $0.id == current.id }) {
                options.insert(current, at: 0)
            }
            let selected = current.flatMap { current in options.firstIndex { $0.id == current.id } }
            let suggested = selected == nil
                ? options.firstIndex { entry.linkCandidates.suggested.contains($0.id) }
                : nil
            return LinkPicker(slot: slot, options: options, selected: selected, suggested: suggested)
        }
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

    /// The line under a use's switch.
    static func note(_ note: UseNote) -> String? {
        switch note {
        case .none: nil
        case .last: L10n.settings_account_use_last()
        case .needsContacts: L10n.settings_account_colleagues_needs_contacts()
        case .withheld: L10n.settings_account_use_withheld()
        case .notOffered: L10n.settings_account_use_not_offered()
        case let .problem(problem, capability): problemNote(problem, calendar: capability == .calendar)
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
