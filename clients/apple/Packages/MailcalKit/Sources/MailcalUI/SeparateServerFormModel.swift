// What one separate-server form holds while it is on screen, in setup and on the account's page
// alike, so the two cannot drift: the login and password typed, and whether the password route is
// drawn beside a sign-in. Connect, Skip and the browser sign-in are changes on the account's page,
// which the core runs and draws on its section; the form ends once the core draws none. The
// decisions are `SeparateServerRules` and `SeparateServerSignIn`.

import Foundation
import MailcalBindings
import Observation

@MainActor
@Observable
final class SeparateServerFormModel {
    var username: String
    var password = ""
    private(set) var signIn = SeparateServerSignIn()
    /// The section as the core last drew it.
    private(set) var section: AccountSeparateServer

    init(_ ask: SeparateServerAsk) {
        username = ask.username
        section = ask.section
        signIn.drawn(ask.section, ask.section, started: false)
    }

    var canConnect: Bool { SeparateServerRules.canConnect(username, password) }

    /// The page's Cancel: held while a change the form started runs, which leaving would not
    /// stop, and while the core completes a sign-in, which it no longer lets go of.
    var canLeave: Bool {
        !section.connect.running && !section.stopUsing.running
            && (section.browser == nil || cancelsOnLeaving != nil)
    }

    /// What is said under the fields: why the last Connect, Skip or sign-in left the uses where
    /// they are.
    var said: String? {
        section.failed.map(AccountSettingsLabels.refused)
    }

    /// The core drew the section again.
    func drawn(_ ask: SeparateServerAsk) {
        let started = SeparateServerRules.passwordStarted(
            ask, username: username, password: password)
        signIn.drawn(section, ask.section, started: started)
        section = ask.section
    }

    func choosePassword() {
        signIn.choosePassword()
    }

    /// Connect: the typed password, tried by the core.
    func connect(_ model: MailboxModel, _ ask: SeparateServerAsk) {
        guard section.connect.enabled, canConnect else { return }
        model.changeAccount(
            .connectSeparateServer(
                account: ask.account, username: username.trimmingCharacters(in: .whitespaces),
                password: TypedPassword(text: password)))
    }

    /// Setup's Skip: the account stops being used for what needs the sign-in.
    func skip(_ model: MailboxModel, _ ask: SeparateServerAsk) {
        guard section.stopUsing.enabled else { return }
        model.changeAccount(.stopUsingSeparateServer(account: ask.account))
    }

    /// The sign-in the server offers, in the browser, superseding one still out.
    func startSignIn(_ model: MailboxModel, _ ask: SeparateServerAsk) {
        guard section.signInControl.enabled else { return }
        model.signInSeparateServer(ask.account)
    }

    /// Cancel beside the line while the browser holds the sign-in.
    func cancelSignIn(_ model: MailboxModel) {
        guard let browser = SeparateServerRules.waiting(section), browser.cancel.enabled else {
            return
        }
        model.cancelSignIn(browser)
    }

    /// The sign-in leaving the form cancels: one still out, unless the core is completing it.
    var cancelsOnLeaving: BrowserSignIn? {
        guard let browser = section.browser, browser.cancel.enabled else { return nil }
        return browser
    }

    /// The form is gone: a sign-in still out goes with it.
    func leave(_ model: MailboxModel) {
        if let browser = cancelsOnLeaving { model.cancelSignIn(browser) }
    }
}
