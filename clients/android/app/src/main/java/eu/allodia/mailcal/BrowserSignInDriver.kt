// A browser sign-in, an existing account's or a separate server's, which the core holds from the
// press to the account it changes (docs/waiting.md, docs/provider-oauth.md rule 13): this host
// opens the browser once for each attempt the core names, and hands back what came to the
// redirect it listens on. Which step the sign-in is at, how long it waits and what it came to are
// the core's. Compose-free and catalog-free, so the JVM suite reaches every decision.
package eu.allodia.mailcal

import java.net.URI
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.SignInBrowserEnd
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/** The Custom Tab a sign-in's page opens in, chosen by where it returns. */
internal enum class SignInBrowser {
    MICROSOFT,
    GOOGLE,

    /** A tab of its own task, as setup's IMAP sign-in opens. */
    IMAP,

    /** A tab of its own task, as setup's JMAP sign-in opens. */
    JMAP,

    /** A Nextcloud server's login page: nothing returns, the core asks the server. */
    NEXTCLOUD,
}

/** The browser to open for sign-in [attempt] of [account]: [url], in [browser]. */
internal data class BrowserOpening(
    val account: String,
    val attempt: ULong,
    val url: String,
    val browser: SignInBrowser,
)

/** The schemes this build's redirects ride, which only a running build can name. */
internal data class RedirectSchemes(val google: String?, val microsoft: String, val app: String)

internal class BrowserSignInDriver {
    /** The attempt whose browser was opened last, so a redraw does not open it again. */
    private var opened: ULong? = null

    /**
     * The browser to open for [record], once for each attempt: null unless it waits on the browser
     * and this host has not opened one for it yet.
     */
    fun toOpen(record: BrowserSignIn?, schemes: RedirectSchemes): BrowserOpening? {
        val stage = record?.stage as? BrowserSignInStage.InBrowser ?: return null
        if (opened == record.attempt) return null
        opened = record.attempt
        val browser = stage.redirectUri?.let { BrowserSignInRules.browserFor(it, schemes) } ?: SignInBrowser.NEXTCLOUD
        return BrowserOpening(record.account, record.attempt, stage.url, browser)
    }
}

internal object BrowserSignInRules {
    /**
     * Where a sign-in of an account of [kind] returns, sent with `AccountPageIntent.SignIn`: the
     * redirect setup uses for that kind; empty for a calendar and contacts account, whose Nextcloud
     * sign-in returns nothing, and for a Google account in a build without a Google registration
     * ([google]), which the core refuses saying the browser did not open. Null for an account
     * that is gone.
     */
    fun redirectFor(kind: CoreAccountKind?, google: String?): String? = when (kind) {
        CoreAccountKind.MICROSOFT -> MicrosoftOAuthConfig.REDIRECT_URI
        CoreAccountKind.GOOGLE -> google.orEmpty()
        CoreAccountKind.IMAP -> ImapOAuthConfig.REDIRECT_URI
        CoreAccountKind.JMAP -> JmapOAuthConfig.REDIRECT_URI
        CoreAccountKind.DAV -> ""
        null -> null
    }

    /**
     * The tab a sign-in returning to [redirectUri] opens in. A JMAP grant may return where it was
     * registered rather than where setup listens now, so anything on this app's scheme that is
     * neither IMAP's nor the Allodia account's opens as JMAP's.
     */
    fun browserFor(redirectUri: String, schemes: RedirectSchemes): SignInBrowser {
        val (scheme, host) = schemeAndHost(redirectUri) ?: return SignInBrowser.JMAP
        return when (OAuthRedirect.of(scheme, host, schemes.google, schemes.microsoft, schemes.app)) {
            OAuthRedirect.MICROSOFT -> SignInBrowser.MICROSOFT
            OAuthRedirect.GOOGLE -> SignInBrowser.GOOGLE
            OAuthRedirect.IMAP -> SignInBrowser.IMAP
            OAuthRedirect.JMAP, OAuthRedirect.ALLODIA, null -> SignInBrowser.JMAP
        }
    }

    /**
     * What a redirect arriving at [url] ([scheme] and [host]) tells the core, when [record] waits
     * on the browser to return there; null when it belongs to a setup flow.
     */
    fun redirected(record: BrowserSignIn?, url: String, scheme: String?, host: String?): AccountPageIntent? {
        val stage = record?.stage as? BrowserSignInStage.InBrowser ?: return null
        val redirect = stage.redirectUri ?: return null
        if (!returnsTo(redirect, scheme, host)) return null
        return AccountPageIntent.SignInBrowserEnded(record.account, record.attempt, SignInBrowserEnd.Redirected(url))
    }

    /**
     * The sign-in a setup flow starting on [redirectUri] cancels: the browser returns to whichever
     * started last, and the core's would otherwise take the setup flow's redirect.
     */
    fun superseded(record: BrowserSignIn?, redirectUri: String): AccountPageIntent? {
        val stage = record?.stage as? BrowserSignInStage.InBrowser ?: return null
        val redirect = stage.redirectUri ?: return null
        val (scheme, host) = schemeAndHost(redirectUri) ?: return null
        if (!returnsTo(redirect, scheme, host)) return null
        return AccountPageIntent.CancelSignIn(record.account, record.attempt)
    }

    /** Whether a URL at [scheme] and [host] returns to [redirectUri]: the same scheme and host. */
    fun returnsTo(redirectUri: String, scheme: String?, host: String?): Boolean {
        val (ownScheme, ownHost) = schemeAndHost(redirectUri) ?: return false
        return scheme != null && scheme.equals(ownScheme, ignoreCase = true) &&
            host.orEmpty().equals(ownHost.orEmpty(), ignoreCase = true)
    }

    private fun schemeAndHost(uri: String): Pair<String, String?>? {
        val parsed = runCatching { URI(uri) }.getOrNull() ?: return null
        val scheme = parsed.scheme ?: return null
        return scheme to parsed.host
    }
}
