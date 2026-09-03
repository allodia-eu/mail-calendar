// Asking a mail server what it accepts, and the browser sign-in when the answer is "sign in".
// The twin of the JMAP half of MainActivityAccounts.kt, and the same division of labour: the core
// owns discovery, registration, PKCE and the exchange, this host owns the browser hop and the
// pending handle it is held on.
//
// One thing differs, and it is why there are three answers rather than two: an IMAP server has no
// HTTP surface to be challenged on, so the core asks the mail server itself what it accepts
// before any credential field is drawn (docs/mail-oauth.md).
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.ImapAuthOffer
import uniffi.mailcal_bindings.ImapLoginRequest

// What this mail server accepts, so the setup card can decide what to ask for rather than
// guessing. BLOCKS (a TLS dial to the mail server, then possibly a few metadata requests), so
// callers await it off the main thread; the setup flow runs it under the "Looking…" spinner that
// is already up, which is what keeps the card from changing shape once it appears.
//
// Never throws: anything short of a usable answer is a password, which is what works everywhere.
internal fun MainActivity.imapAuthOptions(request: ImapLoginRequest): ImapAuthOffer {
    val instance = app ?: return ImapAuthOffer.Password
    return try {
        instance.imapAuthOptions(request)
    } catch (e: Exception) {
        // Not a failure anyone needs to see: the password field is the answer either way.
        logUiInfo("imap auth: the server could not be asked (${e.message}); offering a password")
        ImapAuthOffer.Password
    }
}

// Starts an IMAP sign-in: asks the core for the authorization URL, which runs the whole discovery
// and dynamic-registration chain and so BLOCKS on the network, then opens the browser and holds
// the pending handle until the redirect returns to onNewIntent → completeImapSignIn.
//
// A failure here is expected on a server that does not offer this, so it surfaces as the "use a
// password instead" note rather than a hard error.
internal fun MainActivity.signInWithImap(request: ImapLoginRequest) {
    val activity = this
    val instance = activity.app ?: return
    activity.addError = null
    activity.signingInImap = true
    logUiInfo("imap sign-in: starting")
    thread(name = "mailcal-imap-begin") {
        try {
            val start = instance.beginImapLogin(
                request = request,
                redirectUri = ImapOAuthConfig.REDIRECT_URI,
            )
            activity.mainHandler.post {
                activity.pendingImapLogin = start.pending
                logUiInfo("imap sign-in: opening the browser")
                openImapSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("imap sign-in start failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInImap = false
                // Deliberately the friendly fallback line, not the raw cause: the password field
                // is still right there and still works. The cause is in the file log.
                activity.addError = L10n.setup_imap_signin_failed(activity)
            }
        }
    }
}

// Completes an IMAP sign-in from the redirect [callbackUrl]: exchanges the code for tokens, then
// hands the resulting config TOML to the SAME addAccount path the password form uses. The core
// deliberately returns the identical shape, so there is no second storage path to keep in step.
internal fun MainActivity.completeImapSignIn(callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingImapLogin ?: run {
        // The redirect arrived with nothing armed to receive it: the sign-in was cancelled (or
        // cleared) before the browser came back. Silence here is what makes this invisible.
        logUiWarn("imap sign-in: redirect arrived but no sign-in was pending; ignoring it")
        return
    }
    activity.pendingImapLogin = null
    activity.signingInImap = true
    logUiInfo("imap sign-in: redirect received, completing")
    thread(name = "mailcal-imap-complete") {
        try {
            val configToml = instance.completeImapLogin(pending, callbackUrl)
            logUiInfo("imap sign-in: token exchange succeeded; adding the account")
            activity.mainHandler.post {
                activity.signingInImap = false
                // addAccount owns the connect + persist-after-connect discipline, and reports its
                // own failure through addError.
                activity.addAccount(configToml)
            }
        } catch (e: Exception) {
            logUiWarn("imap sign-in complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInImap = false
                activity.addError = L10n.setup_imap_signin_failed(activity)
            }
        }
    }
}
