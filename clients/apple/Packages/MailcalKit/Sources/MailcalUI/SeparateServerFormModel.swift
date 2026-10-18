// What one separate-server form holds while it is on screen, in setup and on the account's page
// alike, so the two cannot drift: the login and password, the sign-in at the server's own provider,
// and what each answer does. The decisions are `SeparateServerRules` and `SeparateServerSignIn`.

import Foundation
import Observation

@MainActor
@Observable
final class SeparateServerFormModel {
    var username: String
    var password = ""
    var failure: String?
    /// A Connect or a Skip is running in the core.
    var busy = false
    var signIn = SeparateServerSignIn()
    /// The sign-in in flight, held so it can be retired.
    @ObservationIgnored private var task: Task<Void, Never>?

    init(_ ask: SeparateServerAsk) {
        username = ask.username
    }

    /// Whether a core call that nothing interrupts is running, so the form holds its controls.
    var holding: Bool { busy || signIn.completing }

    var canConnect: Bool {
        SeparateServerRules.canConnect(username, password, busy: holding)
    }

    /// Skip and Cancel: live while the browser is out, since each retires the attempt.
    var canLeave: Bool { !holding }

    /// Asks the server whether it can be signed in to at its own provider. The answer is dropped
    /// when the form has moved on (the task was cancelled) and kept only for the form it was for.
    func askOffer(_ model: MailboxModel, _ ask: SeparateServerAsk) async {
        let offers = await model.separateServerOffersSignIn(ask)
        guard !Task.isCancelled else { return }
        signIn.answered(
            SeparateServerOffer(ask, offers: offers),
            started: SeparateServerRules.passwordStarted(
                ask, username: username, password: password))
    }

    func choosePassword() {
        signIn.choosePassword()
    }

    /// Connect: moves the uses with the typed password, retiring a sign-in still out.
    func connect(_ model: MailboxModel, _ ask: SeparateServerAsk, done: @escaping () -> Void) {
        guard canConnect else { return }
        retire()
        busy = true
        failure = nil
        let (login, typed) = (username.trimmingCharacters(in: .whitespaces), password)
        Task {
            let refused = await model.addLinkedCalendarAccount(
                ask, username: login, password: typed)
            busy = false
            settle(SeparateServerRules.afterConnect(refused), done)
        }
    }

    /// Starts a sign-in at the server's own provider, superseding any still out
    /// (docs/provider-oauth.md rule 13).
    func startSignIn(_ model: MailboxModel, _ ask: SeparateServerAsk, done: @escaping () -> Void) {
        guard !holding else { return }
        task?.cancel()
        failure = nil
        let attempt = signIn.begin()
        task = Task {
            let outcome = await model.signInSeparateServer(ask) {
                self.signIn.redirected(attempt)
            }
            // An attempt superseded or retired: the form has moved on without it.
            guard let next = signIn.finish(attempt, outcome) else { return }
            settle(next, done)
        }
    }

    /// A way back from a sign-in still out: the attempt is retired and the form returns to its
    /// offer, saying nothing.
    func retire() {
        task?.cancel()
        task = nil
        signIn.retire()
    }

    private func settle(_ next: SeparateServerNext, _ done: () -> Void) {
        switch next {
        case .done: done()
        case let .stay(reason): failure = reason
        }
    }
}
