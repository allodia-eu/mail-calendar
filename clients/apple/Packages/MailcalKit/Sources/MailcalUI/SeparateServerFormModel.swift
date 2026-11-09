// What one separate-server form holds while it is on screen, in setup and on the account's page
// alike, so the two cannot drift: the login and password, the sign-in at the server's own provider,
// and what each answer does. Connect and Skip are changes on the account's page, which the core
// runs and draws on its section; the form ends once the core draws none. The decisions are
// `SeparateServerRules` and `SeparateServerSignIn`.

import Foundation
import MailcalBindings
import Observation

@MainActor
@Observable
final class SeparateServerFormModel {
    var username: String
    var password = ""
    /// Why the last sign-in in the browser failed; a Connect's failure is the section's.
    var failure: String?
    var signIn = SeparateServerSignIn()
    /// The sign-in with Nextcloud, where the server is Nextcloud.
    let nextcloud = NextcloudSignIn()
    /// The section as the core last drew it.
    private(set) var section: AccountSeparateServer
    /// The sign-in in flight, held so it can be retired.
    @ObservationIgnored private var task: Task<Void, Never>?

    init(_ ask: SeparateServerAsk) {
        username = ask.username
        section = ask.section
    }

    /// A sign-in's own core call is running, which nothing interrupts.
    var completing: Bool { signIn.completing || nextcloud.completing }

    /// Whether the form holds its fields: the core holds Connect, or a sign-in completes.
    var holding: Bool { !section.connect.enabled || completing }

    /// Connect, as the core draws it, held as well while a sign-in completes.
    var connectControl: ControlState { section.connect.held(completing) }

    /// Setup's Skip, likewise.
    var skipControl: ControlState { section.stopUsing.held(completing) }

    var canConnect: Bool { SeparateServerRules.canConnect(username, password) }

    /// Skip and Cancel: live while the browser is out, since each retires the attempt, and held
    /// while a change the form started runs.
    var canLeave: Bool {
        !completing && !section.connect.running && !section.stopUsing.running
    }

    /// What is said under the fields: the sign-in's failure, else the core's.
    var said: String? {
        failure ?? section.failed.map(AccountSettingsLabels.refused)
    }

    /// The core drew the section again. A way in it now offers never takes away a password route
    /// somebody has begun.
    func drawn(_ ask: SeparateServerAsk) {
        if section.signIn == nil, ask.section.signIn != nil {
            signIn.offered(
                started: SeparateServerRules.passwordStarted(
                    ask, username: username, password: password))
        }
        section = ask.section
    }

    func choosePassword() {
        signIn.choosePassword()
    }

    /// Connect: the typed password, tried by the core, retiring a sign-in still out.
    func connect(_ model: MailboxModel, _ ask: SeparateServerAsk) {
        guard connectControl.enabled, canConnect else { return }
        retire()
        failure = nil
        model.changeAccount(
            .connectSeparateServer(
                account: ask.account, username: username.trimmingCharacters(in: .whitespaces),
                password: TypedPassword(text: password)))
    }

    /// Setup's Skip: the account stops being used for what needs the sign-in.
    func skip(_ model: MailboxModel, _ ask: SeparateServerAsk) {
        guard skipControl.enabled else { return }
        retire()
        failure = nil
        model.changeAccount(.stopUsingSeparateServer(account: ask.account))
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

    /// Signs the server in with Nextcloud, moving the uses to the account the grant signs in to.
    /// A grant whose move failed is kept for the next press.
    func startNextcloud(
        _ model: MailboxModel, _ ask: SeparateServerAsk, done: @escaping () -> Void
    ) {
        guard section.connect.enabled, !signIn.completing else { return }
        retireProviderSignIn()
        failure = nil
        nextcloud.start(
            model.app, key: "\(ask.account)|\(ask.uses)",
            begin: { try $0.beginSeparateServerNextcloudSignIn(accountId: ask.account, uses: ask.uses) },
            complete: { grant in
                if let refused = await model.addLinkedNextcloudAccount(ask, grant) {
                    return .failed(said: refused)
                }
                done()
                return .done
            })
    }

    /// A way back from a sign-in still out: the attempt is retired and the form returns to its
    /// offer, saying nothing.
    func retire() {
        retireProviderSignIn()
        nextcloud.cancel()
    }

    /// The form is gone: a sign-in still out retires, and a grant kept for another try is
    /// withdrawn.
    func leave(_ model: MailboxModel) {
        retireProviderSignIn()
        nextcloud.leave(model.app)
    }

    private func retireProviderSignIn() {
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
