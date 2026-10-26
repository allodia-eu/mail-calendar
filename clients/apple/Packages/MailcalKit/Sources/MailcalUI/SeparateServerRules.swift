// A calendar or address book the account's sign-in does not reach (docs/accounts.md rule 18): the
// question a switch to the provider asks first, the step setup shows after the mail sign-in, and
// the sign-in of its own an account's page offers. Decided from the core's answers alone, so each
// rule is tested without a view. The words are the catalog's.

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
/// account's page.
struct SeparateServerAsk: Equatable {
    /// The mailbox whose uses move to an account of their own.
    let account: String
    let uses: [AccountCapability]
    /// The server, host and port only, never a path.
    let host: String
    /// The login the form starts from.
    let username: String

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

    /// The form for `entry`: the uses whose server needs a sign-in of its own, or refused the
    /// account's, as the core names them. `nil` while every use connects. Setup shows it as a
    /// step once the add's own dial has said so, and the account's page under those uses.
    static func ask(_ entry: AccountEntry) -> SeparateServerAsk? {
        guard let server = entry.separateServer, UnreachedSet(server.uses) != nil else {
            return nil
        }
        let login = server.username.trimmingCharacters(in: .whitespaces)
        return SeparateServerAsk(
            account: entry.id, uses: server.uses, host: server.host,
            username: login.isEmpty ? entry.address : login)
    }

    /// Whether the account's page offers the sign-in of its own under `capability`'s switch.
    static func offered(_ entry: AccountEntry, _ capability: AccountCapability) -> Bool {
        ask(entry)?.uses.contains(capability) ?? false
    }

    /// Connect is live with a login and a password, and nothing running.
    static func canConnect(_ username: String, _ password: String, busy: Bool) -> Bool {
        !busy && !password.isEmpty && !username.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// What Skip switches off: exactly the uses the form was for.
    static func skipped(_ ask: SeparateServerAsk) -> [AccountCapability] {
        ask.uses
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
