// What this host still decides about an existing account's browser sign-in, which the core runs:
// where it returns, which tab opens it, that the tab opens once for each attempt, and whether an
// arriving redirect is the core's or a setup flow's.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.SignInBrowserEnd
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class BrowserSignInDriverTest {
    // The application id, which a brand may change (docs/branding.md).
    private val app = JmapOAuthConfig.REDIRECT_SCHEME
    private val google = "com.googleusercontent.apps.123-abc"
    private val schemes = RedirectSchemes(google = google, microsoft = MicrosoftOAuthConfig.REDIRECT_SCHEME, app = app)
    private val imap = "$app://imap-oauth"
    private val jmap = "$app://jmap-oauth"
    private val free = ControlState(enabled = true, running = false)

    private fun record(attempt: ULong, stage: BrowserSignInStage, account: String = "acct") =
        BrowserSignIn(attempt, account, stage, free)

    private fun waiting(attempt: ULong, redirect: String?) =
        record(attempt, BrowserSignInStage.InBrowser("https://auth.example.org/$attempt", redirect))

    @Test
    fun a_sign_in_names_the_redirect_setup_listens_on_for_its_kind() {
        val googleRedirect = "$google:/oauth2redirect"
        assertEquals(MicrosoftOAuthConfig.REDIRECT_URI, BrowserSignInRules.redirectFor(CoreAccountKind.MICROSOFT, googleRedirect))
        assertEquals(googleRedirect, BrowserSignInRules.redirectFor(CoreAccountKind.GOOGLE, googleRedirect))
        assertEquals(ImapOAuthConfig.REDIRECT_URI, BrowserSignInRules.redirectFor(CoreAccountKind.IMAP, googleRedirect))
        assertEquals(JmapOAuthConfig.REDIRECT_URI, BrowserSignInRules.redirectFor(CoreAccountKind.JMAP, googleRedirect))
        // A Nextcloud sign-in returns nothing: the core asks the server.
        assertEquals("", BrowserSignInRules.redirectFor(CoreAccountKind.DAV, googleRedirect))
        // A build without a Google registration names nowhere, which the core refuses.
        assertEquals("", BrowserSignInRules.redirectFor(CoreAccountKind.GOOGLE, null))
        assertNull(BrowserSignInRules.redirectFor(null, googleRedirect))
    }

    @Test
    fun the_browser_opens_once_for_each_attempt_and_only_while_the_core_waits_on_it() {
        val driver = BrowserSignInDriver()
        assertNull(driver.toOpen(null, schemes))
        assertNull(driver.toOpen(record(1uL, BrowserSignInStage.Starting), schemes))
        assertNull(driver.toOpen(record(1uL, BrowserSignInStage.Asking(emptyList())), schemes))

        val opening = driver.toOpen(waiting(1uL, imap), schemes)
        assertEquals(BrowserOpening("acct", 1uL, "https://auth.example.org/1", SignInBrowser.IMAP), opening)
        assertNull("a redraw does not open it again", driver.toOpen(waiting(1uL, imap), schemes))
        assertNull(driver.toOpen(record(1uL, BrowserSignInStage.Completing), schemes))

        // A press while the browser waited superseded it: the new attempt opens a tab of its own.
        assertEquals(2uL, driver.toOpen(waiting(2uL, imap), schemes)?.attempt)
    }

    @Test
    fun each_redirect_opens_in_the_tab_setup_uses_for_it() {
        val driver = BrowserSignInDriver()
        fun browserOf(attempt: ULong, redirect: String?) = driver.toOpen(waiting(attempt, redirect), schemes)?.browser

        assertEquals(SignInBrowser.MICROSOFT, browserOf(1uL, MicrosoftOAuthConfig.REDIRECT_URI))
        assertEquals(SignInBrowser.GOOGLE, browserOf(2uL, "$google:/oauth2redirect"))
        assertEquals(SignInBrowser.IMAP, browserOf(3uL, imap))
        assertEquals(SignInBrowser.JMAP, browserOf(4uL, jmap))
        // A JMAP grant returns where it was registered, which may not be where setup listens now.
        assertEquals(SignInBrowser.JMAP, browserOf(5uL, "$app://oauth-callback"))
        assertEquals(SignInBrowser.NEXTCLOUD, browserOf(6uL, null))
    }

    @Test
    fun a_redirect_goes_to_the_core_only_when_its_sign_in_waits_there() {
        val url = "$app://imap-oauth?code=abc&state=xyz"
        val sent = BrowserSignInRules.redirected(waiting(3uL, imap), url, app, "imap-oauth")
        assertEquals(AccountPageIntent.SignInBrowserEnded("acct", 3uL, SignInBrowserEnd.Redirected(url)), sent)

        // The JMAP host is setup's while the core waits on IMAP's, and nothing waits on Allodia's.
        assertNull(BrowserSignInRules.redirected(waiting(3uL, imap), "$app://jmap-oauth?code=a", app, "jmap-oauth"))
        assertNull(BrowserSignInRules.redirected(waiting(3uL, imap), "$app://account-oauth?code=a", app, "account-oauth"))
        // Nothing waits, or nothing returns: every redirect is a setup flow's.
        assertNull(BrowserSignInRules.redirected(null, url, app, "imap-oauth"))
        assertNull(BrowserSignInRules.redirected(record(3uL, BrowserSignInStage.Starting), url, app, "imap-oauth"))
        assertNull(BrowserSignInRules.redirected(waiting(3uL, null), url, app, "imap-oauth"))
    }

    @Test
    fun a_google_redirect_matches_by_scheme_alone_since_it_has_no_host() {
        val redirect = "$google:/oauth2redirect"
        assertTrue(BrowserSignInRules.returnsTo(redirect, google, null))
        assertFalse(BrowserSignInRules.returnsTo(redirect, app, null))
        assertFalse(BrowserSignInRules.returnsTo(redirect, null, null))
        assertTrue(BrowserSignInRules.returnsTo(MicrosoftOAuthConfig.REDIRECT_URI, "msauth", app))
    }

    @Test
    fun a_setup_sign_in_on_the_same_redirect_cancels_the_one_waiting_there() {
        assertEquals(
            AccountPageIntent.CancelSignIn("acct", 5uL),
            BrowserSignInRules.superseded(waiting(5uL, imap), ImapOAuthConfig.REDIRECT_URI),
        )
        assertNull(BrowserSignInRules.superseded(waiting(5uL, imap), JmapOAuthConfig.REDIRECT_URI))
        assertNull(BrowserSignInRules.superseded(waiting(5uL, null), ImapOAuthConfig.REDIRECT_URI))
        assertNull(BrowserSignInRules.superseded(record(5uL, BrowserSignInStage.Completing), ImapOAuthConfig.REDIRECT_URI))
    }
}
