// The JMAP browser sign-in: a new account's from the setup card, and an existing account's again
// from the expired-sign-in banner. One slot holds both (MainActivity.jmapSignIn), because both come
// back through one redirect host, and a press of either supersedes whatever the slot held
// (docs/provider-oauth.md rule 13).
package eu.allodia.mailcal

import kotlin.concurrent.thread

/** What a JMAP sign-in is for, which decides what its redirect completes. */
internal sealed interface JmapFlow {
    /** A first sign-in from the setup card, ending in a new account. */
    data object NewAccount : JmapFlow

    /** An existing account signed in again, from the expired-sign-in banner. */
    data class Reauth(val accountId: String) : JmapFlow
}

// Starts a JMAP sign-in: asks the core for the authorization URL, which runs the whole discovery
// + dynamic-registration chain, so this BLOCKS on the network and must run off the main thread:
// then opens the browser and holds the pending handle until the redirect returns to onNewIntent →
// completeJmapRedirect.
//
// Unlike Microsoft/Google there is no client id to pass: the core registers this install with the
// discovered server itself (RFC 7591). A failure here is expected on servers that don't support
// this, so it surfaces as the "use a password or API token instead" note rather than a hard error.
internal fun MainActivity.signInWithJmap(email: String, serverUrl: String?) {
    val activity = this
    val instance = activity.app ?: return
    activity.addError = null
    val attempt = activity.jmapSignIn.begin(JmapFlow.NewAccount)
    logUiInfo("jmap sign-in: starting (server hint: ${serverUrl?.ifBlank { null } ?: "derived from the email domain"})")
    thread(name = "mailcal-jmap-begin") {
        try {
            val start = instance.beginJmapLogin(
                email = email,
                serverUrl = serverUrl?.takeIf { it.isNotBlank() },
                redirectUri = JmapOAuthConfig.REDIRECT_URI,
            )
            activity.mainHandler.post {
                if (!activity.jmapSignIn.opening(attempt, start.pending)) {
                    logUiInfo("jmap sign-in: left or superseded before the browser opened")
                    return@post
                }
                logUiInfo("jmap sign-in: opening the browser")
                openJmapSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("jmap sign-in start failed: ${e.message}")
            activity.mainHandler.post {
                if (!activity.jmapSignIn.settled(attempt)) return@post
                // Deliberately the friendly fallback line, not the raw cause: the manual secret
                // field is still right there and still works. The cause is in the file log.
                activity.addError = L10n.setup_jmap_signin_failed(activity)
            }
        }
    }
}

// Signs an existing OAuth JMAP account back in, from the expired-sign-in banner. Unlike
// signInWithJmap this makes no discovery calls: the core builds the authorization URL from that
// account's own persisted grant, so nothing is re-registered and the same client id is replayed.
// It still runs off the main thread, because the two paths are otherwise identical from here. A
// press while a sign-in is out supersedes it: a tab closed without signing in sends nothing back.
internal fun MainActivity.reconnectJmap(accountId: String) {
    val activity = this
    val instance = activity.app ?: return
    val attempt = activity.jmapSignIn.begin(JmapFlow.Reauth(accountId))
    logUiInfo("jmap sign-in: re-authenticating an existing account")
    thread(name = "mailcal-jmap-reauth-begin") {
        try {
            val start = instance.beginJmapReauth(accountId)
            activity.mainHandler.post {
                if (!activity.jmapSignIn.opening(attempt, start.pending)) {
                    logUiInfo("jmap sign-in: re-authentication superseded before the browser opened")
                    return@post
                }
                logUiInfo("jmap sign-in: opening the browser to re-authenticate")
                openJmapSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("jmap re-authentication start failed: ${e.message}")
            activity.mainHandler.post {
                if (activity.jmapSignIn.settled(attempt)) activity.toastSignInFailed()
            }
        }
    }
}

// The redirect [callbackUrl] for whichever JMAP sign-in the slot holds.
internal fun MainActivity.completeJmapRedirect(callbackUrl: String) {
    val landed = jmapSignIn.redirected() ?: run {
        // The redirect arrived with nothing armed to receive it: the sign-in was superseded or
        // left before the browser came back. Silence here is what made this invisible the first
        // time it happened, so say so.
        logUiWarn("jmap sign-in: redirect arrived but no sign-in was pending; ignoring it")
        return
    }
    when (val flow = landed.flow) {
        JmapFlow.NewAccount -> completeJmapSignIn(landed, callbackUrl)
        is JmapFlow.Reauth -> completeJmapReauth(flow.accountId, landed, callbackUrl)
    }
}

// Completes a JMAP sign-in: exchanges the code for tokens, then hands the resulting config TOML to
// the SAME addAccount + SecureStore path the manual JMAP form uses, the core deliberately returns
// the identical `[jmap]` shape, so there is no second storage path to keep in step. Off the main
// thread (the exchange and connect both block). The card stays busy throughout.
private fun MainActivity.completeJmapSignIn(landed: Landed<JmapFlow>, callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    logUiInfo("jmap sign-in: redirect received, completing")
    thread(name = "mailcal-jmap-complete") {
        try {
            val configToml = instance.completeJmapLogin(landed.handle, callbackUrl)
            logUiInfo("jmap sign-in: token exchange succeeded; adding the account")
            activity.mainHandler.post {
                if (!activity.jmapSignIn.settled(landed.attempt)) {
                    logUiInfo("jmap sign-in: left while the code was exchanged; not adding it")
                    return@post
                }
                // addAccount owns the connect + persist-after-connect discipline, and reports its
                // own failure through addError.
                activity.addAccount(configToml)
            }
        } catch (e: Exception) {
            logUiWarn("jmap sign-in complete failed: ${e.message}")
            activity.mainHandler.post {
                if (!activity.jmapSignIn.settled(landed.attempt)) return@post
                activity.addError = L10n.setup_jmap_signin_failed(activity)
            }
        }
    }
}

// Completes a JMAP RE-authentication. There is no addAccount here: [accountId] already exists, so
// the core connects with the fresh grant, writes it to the secure store through the same port a
// token rotation uses, and retracts the expired-sign-in prompt: which is what makes the banner
// disappear. Off the main thread (the exchange and connect block).
private fun MainActivity.completeJmapReauth(
    accountId: String,
    landed: Landed<JmapFlow>,
    callbackUrl: String,
) {
    val activity = this
    val instance = activity.app ?: return
    logUiInfo("jmap sign-in: redirect received, completing the re-authentication")
    thread(name = "mailcal-jmap-reauth-complete") {
        try {
            instance.completeJmapReauth(accountId, landed.handle, callbackUrl)
            logUiInfo("jmap sign-in: the account is connected again")
            activity.mainHandler.post { activity.jmapSignIn.settled(landed.attempt) }
        } catch (e: Exception) {
            logUiWarn("jmap re-authentication complete failed: ${e.message}")
            activity.mainHandler.post {
                if (activity.jmapSignIn.settled(landed.attempt)) activity.toastSignInFailed()
            }
        }
    }
}

// The one thing a failed re-authentication can say on the mailbox screen: plain words, because the
// cause is an OAuth protocol string that belongs in the log, not in front of anyone. The banner is
// still up (the core leaves the prompt raised on every failure), so the retry is one tap away.
private fun MainActivity.toastSignInFailed() {
    android.widget.Toast
        .makeText(this, L10n.signin_expired_failed(this), android.widget.Toast.LENGTH_LONG)
        .show()
}
