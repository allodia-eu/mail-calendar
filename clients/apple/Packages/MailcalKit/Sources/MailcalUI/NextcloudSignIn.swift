// One surface's sign-in with Nextcloud: the attempt the browser holds, and the grant kept until
// it is an account (docs/nextcloud-sign-in.md rule 6). The server hands a grant out once, so one
// whose completion failed is kept and the next press tries it again without the browser; one the
// surface no longer wants is withdrawn, but never while the core is completing it.

import Foundation
import MailcalBindings
import Observation

/// How a surface's completion of a grant ended.
enum NextcloudCompleted: Equatable {
    case done
    /// The grant is kept for the next press. `said` is drawn under the sign-in, or `nil` when the
    /// surface says it somewhere of its own.
    case failed(said: String?)
}

/// The decisions, apart from the tasks that act on them, so each is tested without a core.
struct NextcloudSignInState: Equatable {
    /// A grant no account was made of yet, and what it was for: a server, a mailbox's uses, or an
    /// account.
    struct Held: Equatable {
        let key: String
        let grant: NextcloudGrant
    }

    /// What became of an attempt that ended.
    enum Landing: Equatable {
        /// Granted to the attempt in flight: complete it.
        case complete(NextcloudGrant)
        /// Granted to an attempt superseded or retired: nobody will use it.
        case withdraw(NextcloudGrant)
        /// Ended without a grant, for the attempt in flight.
        case ended(NextcloudFlightEnd)
        /// Ended without a grant, for an attempt nobody waits on.
        case dropped
    }

    private(set) var attempts = SignInAttempts()
    private(set) var held: Held?
    /// The core is putting the held grant on an account.
    private(set) var completing = false
    /// The surface was left while the core completed: the grant is withdrawn if it fails.
    private(set) var leftWhileCompleting = false

    var waiting: Bool { attempts.signingIn }

    /// The grant kept for `key`, taken for another try; `nil` when none is kept for it, or one is
    /// being completed already.
    mutating func retry(_ key: String?) -> NextcloudGrant? {
        guard !completing, let held, key == nil || held.key == key else { return nil }
        completing = true
        leftWhileCompleting = false
        return held.grant
    }

    /// Starts the browser for a new attempt, superseding one in flight. A grant kept for anything
    /// else is answered to be withdrawn: the person moved on from it.
    mutating func begin() -> (attempt: Int, withdraw: NextcloudGrant?) {
        let withdraw = held?.grant
        held = nil
        return (attempts.begin(), withdraw)
    }

    mutating func landed(_ attempt: Int, key: String, _ end: NextcloudFlightEnd) -> Landing {
        guard attempts.finish(attempt) else {
            if case let .granted(grant) = end { return .withdraw(grant) }
            return .dropped
        }
        guard case let .granted(grant) = end else { return .ended(end) }
        held = Held(key: key, grant: grant)
        completing = true
        leftWhileCompleting = false
        return .complete(grant)
    }

    /// The completion answered. Answers a grant to withdraw: one that failed after its surface
    /// was left.
    mutating func completed(_ result: NextcloudCompleted) -> NextcloudGrant? {
        completing = false
        if result == .done {
            held = nil
            return nil
        }
        guard leftWhileCompleting else { return nil }
        leftWhileCompleting = false
        defer { held = nil }
        return held?.grant
    }

    /// The person took a way back from the browser: the attempt retires, saying nothing.
    mutating func retire() {
        attempts.escape()
    }

    /// The surface is left. Answers the grant to withdraw now; one the core is completing is
    /// withdrawn only if that completion fails.
    mutating func leave() -> NextcloudGrant? {
        retire()
        if completing {
            leftWhileCompleting = true
            return nil
        }
        defer { held = nil }
        return held?.grant
    }
}

/// A surface's sign-in with Nextcloud, observed by its view.
@MainActor
@Observable
final class NextcloudSignIn {
    private(set) var state = NextcloudSignInState()
    /// What the last attempt or completion said, drawn under the sign-in.
    var failure: String?
    @ObservationIgnored private var task: Task<Void, Never>?

    var waiting: Bool { state.waiting }
    var completing: Bool { state.completing }
    var busy: Bool { waiting || completing }

    /// Signs in for `key`: with the grant kept for it when there is one, else through the
    /// browser, `begin` starting the flight off the main actor. `complete` makes the account.
    func start(
        _ app: MailcalApp?,
        key: String,
        begin: @escaping @Sendable (MailcalApp) throws -> NextcloudSignInStart,
        complete: @escaping (NextcloudGrant) async -> NextcloudCompleted
    ) {
        guard let app, !state.completing else { return }
        failure = nil
        if let grant = state.retry(key) {
            task = Task { await finish(app, grant, complete) }
            return
        }
        task?.cancel()
        let (attempt, withdraw) = state.begin()
        if let withdraw { app.abandonNextcloudSignIn(grant: withdraw.value) }
        task = Task {
            let end = await flyNextcloudSignIn(app, begin: begin)
            switch state.landed(attempt, key: key, end) {
            case let .complete(grant):
                await finish(app, grant, complete)
            case let .withdraw(grant):
                app.abandonNextcloudSignIn(grant: grant.value)
            case let .ended(end):
                failure = end.said
            case .dropped:
                break
            }
        }
    }

    /// Completes the grant kept for whatever this surface signed in for, with `complete`: an
    /// account already set up taking a grant its add was refused. Answers whether one was kept.
    @discardableResult
    func completeHeld(
        _ app: MailcalApp?, _ complete: @escaping (NextcloudGrant) async -> NextcloudCompleted
    ) async -> Bool {
        guard let app, let grant = state.retry(nil) else { return false }
        await finish(app, grant, complete)
        return true
    }

    /// Cancel while the browser is out: the attempt retires, and a grant it still brings is
    /// withdrawn by the flight.
    func cancel() {
        guard !state.completing else { return }
        task?.cancel()
        task = nil
        state.retire()
    }

    /// The surface is left: the attempt retires and a grant kept for another try is withdrawn.
    func leave(_ app: MailcalApp?) {
        if !state.completing {
            task?.cancel()
            task = nil
        }
        failure = nil
        if let grant = state.leave() { app?.abandonNextcloudSignIn(grant: grant.value) }
    }

    private func finish(
        _ app: MailcalApp, _ grant: NextcloudGrant,
        _ complete: (NextcloudGrant) async -> NextcloudCompleted
    ) async {
        let result = await complete(grant)
        if let withdraw = state.completed(result) {
            app.abandonNextcloudSignIn(grant: withdraw.value)
        }
        if case let .failed(said) = result { failure = said }
    }
}

/// What the manual calendar and contacts form knows about Nextcloud: the server it last asked
/// about, so leaving a field twice asks once, and the server that answered yes.
struct NextcloudManualAsk: Equatable {
    private(set) var asked: String?
    private(set) var offered: String?

    /// The server the form names: its calendar server, else its address book's.
    static func server(_ caldav: String, _ carddav: String) -> String? {
        [caldav, carddav]
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty }
    }

    /// A server field was left: the server to ask about, when it names one not asked about yet.
    mutating func question(_ caldav: String, _ carddav: String) -> String? {
        guard let server = Self.server(caldav, carddav), server != asked else { return nil }
        asked = server
        return server
    }

    /// The server answered.
    mutating func answered(_ server: String, offers: Bool) {
        guard server == asked else { return }
        offered = offers ? server : nil
    }

    /// The server to offer the sign-in at, while the form still names the one that said yes.
    func offer(_ caldav: String, _ carddav: String) -> String? {
        guard let offered, Self.server(caldav, carddav) == offered else { return nil }
        return offered
    }
}
