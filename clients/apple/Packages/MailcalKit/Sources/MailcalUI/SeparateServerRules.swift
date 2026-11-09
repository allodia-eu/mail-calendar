// A calendar or address book the account's sign-in does not reach (docs/accounts.md rule 18): the
// question a switch to the provider asks first, and the form, setup's step after the mail sign-in
// and the account page's, that gives them a sign-in of their own from the core's section. Decided
// from the core's answers alone, so each rule is tested without a view. The words are the
// catalog's.

import Foundation
import MailcalBindings

/// Which of calendar and contacts a sign-in does not reach, as the copy names them.
enum UnreachedSet: Equatable {
    case both
    case calendar
    case contacts

    /// The set `uses` names; `nil` when it names neither calendar nor contacts.
    init?(_ uses: [AccountCapability]) {
        switch (uses.contains(.calendar), uses.contains(.contacts)) {
        case (true, true): self = .both
        case (true, false): self = .calendar
        case (false, true): self = .contacts
        case (false, false): return nil
        }
    }
}

/// What the person answered when a switch to the provider would not reach their calendar or
/// contacts.
enum UnreachedAnswer: Equatable {
    case keep
    case stop
    case cancel

    var decision: UnreachedDecision {
        switch self {
        case .keep: .proceed(.keepOnPassword)
        case .stop: .proceed(.stopUsing)
        case .cancel: .abandon
        }
    }
}

/// Whether a sign-in goes on, and the answer the core is handed when it does.
enum UnreachedDecision: Equatable {
    case proceed(UnreachedUses?)
    /// The browser never opens and nothing changes.
    case abandon
}

/// Asks the person, on the main actor, what becomes of the uses a sign-in does not reach.
typealias UnreachedAsker = @MainActor (UnreachedSet) async -> UnreachedAnswer

/// The form that signs in to a calendar or address book server of its own, in setup and on the
/// account's page, as the core last drew it.
struct SeparateServerAsk: Equatable {
    /// The mailbox whose uses move to an account of their own.
    let account: String
    let section: AccountSeparateServer

    var uses: [AccountCapability] { section.uses }
    /// The server, host and port only, never a path.
    var host: String { section.host }
    /// The login the form starts from.
    var username: String { section.username }
    var set: UnreachedSet? { UnreachedSet(uses) }
}

enum SeparateServerRules {
    /// Whether a sign-in asks first, and as which set: only when it names calendar or contacts.
    static func asks(_ unreached: [AccountCapability]) -> UnreachedSet? {
        UnreachedSet(unreached)
    }

    /// What a sign-in hands the core, given what it said was unreached and how the person
    /// answered. Nothing unreached goes on with no answer, as before this question existed.
    static func decision(
        _ unreached: [AccountCapability], answer: UnreachedAnswer?
    ) -> UnreachedDecision {
        guard asks(unreached) != nil else { return .proceed(nil) }
        return answer?.decision ?? .abandon
    }

    /// The form for `account`'s `section`, the core's for a mailbox whose calendar or contacts need
    /// a sign-in of their own. `nil` while nothing needs one. Setup shows it as a step once the
    /// add's own dial has said so, and the account's page under the use the core names.
    static func ask(_ account: String, _ section: AccountSeparateServer?) -> SeparateServerAsk? {
        guard let section, UnreachedSet(section.uses) != nil else { return nil }
        return SeparateServerAsk(account: account, section: section)
    }

    /// Whether the fields hold what Connect sends: a login and a password.
    static func canConnect(_ username: String, _ password: String) -> Bool {
        !password.isEmpty && !username.trimmingCharacters(in: .whitespaces).isEmpty
    }
}

/// The catalog's words for the decisions above.
enum SeparateServerLabels {
    /// The title of the question a switch to the provider asks first.
    static func unreachedTitle(_ set: UnreachedSet) -> String {
        switch set {
        case .both: L10n.signin_unreached_title_both()
        case .calendar: L10n.signin_unreached_title_calendar()
        case .contacts: L10n.signin_unreached_title_contacts()
        }
    }

    /// The title of the form that signs in to the separate server.
    static func formTitle(_ set: UnreachedSet) -> String {
        switch set {
        case .both: L10n.separate_server_title_both()
        case .calendar: L10n.separate_server_title_calendar()
        case .contacts: L10n.separate_server_title_contacts()
        }
    }

    /// The form's body, which names the server.
    static func formBody(_ ask: SeparateServerAsk) -> String {
        L10n.separate_server_body(host: ask.host)
    }
}
