// An existing account's browser sign-in, which the core holds from the press to the account it
// changes (docs/waiting.md, docs/provider-oauth.md rule 13): sign in again, a switch from a
// password to the provider or to Nextcloud, and a use the provider is asked for. Every surface
// asks the core for one; this host opens the browser once for each attempt the core names,
// listens where it returns and hands back what came. Which step it is at, what it waits for and
// what it came to are the core's, so nothing here guards a press or keeps a timer.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Signs `account` in through its browser sign-in, asking its provider for `adding` as well:
    /// again, or in place of its password, whichever its page offers. It supersedes a sign-in
    /// still out.
    func signInThroughBrowser(_ account: String, adding: [AccountCapability] = []) {
        guard let kind = accountEntry(account)?.kind else { return }
        let google = oauthRoutes().googleRedirectUri
        if let redirect = AccountSignInRules.redirectURI(kind, google: google) {
            startSignIn(account, adding, redirect: redirect)
            return
        }
        #if os(macOS)
        Task { await self.signInThroughLoopback(account, adding) }
        #else
        // A Google account in a build without a Google registration: nothing here can be
        // returned to, which the core says once its browser does not open.
        startSignIn(account, adding, redirect: "")
        #endif
    }

    /// Cancels the sign-in `browser` is, from its Cancel.
    func cancelSignIn(_ browser: BrowserSignIn) {
        changeAccount(.cancelSignIn(account: browser.account, attempt: browser.attempt))
    }

    /// Hands the core the answer to sign-in `attempt`'s question.
    func answerSignInQuestion(_ account: String, attempt: UInt64, _ answer: UnreachedAnswer) {
        changeAccount(
            .answerUnreached(
                account: account, attempt: attempt, answer: AccountSignInRules.answer(answer)))
    }

    /// Opens the browser for the core's sign-in once it waits on one, and closes one the core no
    /// longer waits on. Run on every Settings signal.
    func driveBrowserSignIn() {
        guard let app else { return }
        let record = app.browserSignIn()
        if let held = signInBrowser,
           !AccountSignInRules.keeps(record, attempt: held.attempt, opened: held.opened)
        {
            held.close()
            signInBrowser = nil
        }
        guard let record,
              let opening = AccountSignInRules.opening(
                  record, opened: signInBrowser?.opened ?? false)
        else { return }
        let browser = signInBrowser ?? SignInBrowser(attempt: record.attempt)
        signInBrowser = browser
        let (account, attempt) = (record.account, record.attempt)
        browser.open(url: opening.url, redirect: opening.redirect) { [weak self] end in
            self?.changeAccount(.signInBrowserEnded(account: account, attempt: attempt, end: end))
        }
    }

    private func startSignIn(_ account: String, _ adding: [AccountCapability], redirect: String) {
        changeAccount(.signIn(account: account, adding: adding, redirectUri: redirect))
        driveBrowserSignIn()
    }

    #if os(macOS)
    /// A Google sign-in on the Mac returns to a loopback port, bound before the core is asked so
    /// the core builds its sign-in for that port.
    private func signInThroughLoopback(_ account: String, _ adding: [AccountCapability]) async {
        let flow = GoogleLoopbackFlow()
        guard let redirect = try? await flow.redirectURI() else {
            // The flow logged why. The core says the browser did not open.
            startSignIn(account, adding, redirect: "")
            return
        }
        changeAccount(.signIn(account: account, adding: adding, redirectUri: redirect))
        guard let attempt = app?.browserSignIn()?.attempt else {
            flow.cancel()
            return
        }
        signInBrowser?.close()
        signInBrowser = SignInBrowser(attempt: attempt, loopback: (redirect: redirect, flow: flow))
        driveBrowserSignIn()
    }
    #endif
}
