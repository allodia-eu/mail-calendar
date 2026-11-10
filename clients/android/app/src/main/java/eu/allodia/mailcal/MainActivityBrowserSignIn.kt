// An existing account's browser sign-in, from its page or a notice, which the core runs
// (BrowserSignInDriver.kt): this host names where the browser returns, opens it once for each
// attempt the core names, and hands back the redirect. A Custom Tab tells nobody it was closed,
// so the page's Cancel and the core's cap on the wait end one the person left.
package eu.allodia.mailcal

import android.net.Uri
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.SignInBrowserEnd

private fun redirectSchemes() = RedirectSchemes(
    google = GoogleOAuthConfig.REDIRECT_SCHEME,
    microsoft = MicrosoftOAuthConfig.REDIRECT_SCHEME,
    app = JmapOAuthConfig.REDIRECT_SCHEME,
)

/**
 * Signs [accountId] in through its browser sign-in, asking its provider for [adding] besides what
 * it is used for. It supersedes one still out.
 */
internal fun MainActivity.signInInBrowser(accountId: String, adding: List<AccountCapability>) {
    val redirect = BrowserSignInRules.redirectFor(accountSettings.entry(accountId)?.kind, GoogleOAuthConfig.REDIRECT_URI)
        ?: return logUiWarn("sign-in: asked for an account that is gone")
    changeAccount(AccountPageIntent.SignIn(accountId, adding, redirect))
}

/** Opens the browser the core's sign-in waits on, once for each attempt. On every Settings signal. */
internal fun MainActivity.driveBrowserSignIn() {
    val instance = app ?: return
    val opening = browserSignIns.toOpen(instance.browserSignIn(), redirectSchemes()) ?: return
    logUiInfo("sign-in: opening the browser for an account's sign-in")
    runCatching { openSignInBrowser(opening) }.onFailure {
        logUiWarn("sign-in: the browser did not open: ${it.message}")
        changeAccount(AccountPageIntent.SignInBrowserEnded(opening.account, opening.attempt, SignInBrowserEnd.NotOpened))
    }
}

private fun MainActivity.openSignInBrowser(opening: BrowserOpening) = when (opening.browser) {
    SignInBrowser.MICROSOFT -> openMicrosoftSignIn(this, opening.url)
    SignInBrowser.GOOGLE -> openGoogleSignIn(this, opening.url)
    SignInBrowser.IMAP -> openImapSignIn(this, opening.url)
    SignInBrowser.JMAP -> openJmapSignIn(this, opening.url)
    SignInBrowser.NEXTCLOUD -> openNextcloudPage(this, opening.url)
}

/** Hands [data] to the core's sign-in when it waits there; false when it is a setup flow's. */
internal fun MainActivity.redirectToSignIn(data: Uri): Boolean {
    val instance = app ?: return false
    val intent = BrowserSignInRules.redirected(instance.browserSignIn(), data.toString(), data.scheme, data.host)
        ?: return false
    logUiInfo("sign-in: the browser returned to an account's sign-in")
    changeAccount(intent)
    return true
}

/**
 * A setup flow opening its browser on [redirectUri] takes that redirect from an account's sign-in
 * waiting on it: the browser returns to whichever started last.
 */
internal fun MainActivity.yieldBrowserSignIn(redirectUri: String) {
    val instance = app ?: return
    BrowserSignInRules.superseded(instance.browserSignIn(), redirectUri)?.let {
        logUiInfo("sign-in: a setup sign-in took the redirect an account's sign-in waited on")
        changeAccount(it)
    }
}
