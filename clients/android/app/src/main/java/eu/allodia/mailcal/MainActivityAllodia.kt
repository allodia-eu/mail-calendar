// Signing in to an Allodia account, driven from the activity: begin → browser → complete, plus
// signing out. The twin of the JMAP sign-in in MainActivityCore.kt, and it holds no token of its
// own, the core exchanges the code, asks the service whose account it is, and writes the grant
// through the same SecureStore every mail account uses.
//
// Both core calls BLOCK on the network, so both run off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.allodiaSignInAvailable

// Whether this build carries the route at all. False is the ordinary answer for a build from
// source, and the settings card then draws nothing rather than a button that dead-ends.
internal fun allodiaSignInOffered(): Boolean = allodiaSignInAvailable()

// Starts a sign-in: asks the core for the authorization URL, which reads the service's own OAuth
// metadata, so it blocks, then opens the browser and holds the pending handle until the redirect
// returns to onNewIntent → completeAllodiaSignIn.
internal fun MainActivity.signInToAllodia() = beginAllodia(create = false)

// Starts a registration: the same flow asking the service for its sign-up page instead. The
// redirect, the exchange and the store are identical, so onNewIntent needs to know nothing about
// which of the two started it.
internal fun MainActivity.registerAllodiaAccount() = beginAllodia(create = true)

// Opens the page where someone changes their details or deletes the account. Nothing is pending
// and nothing comes back: this is a page, not a flow.
internal fun MainActivity.manageAllodiaAccount() {
    val url = app?.allodiaAccountUrl() ?: return
    logUiInfo("allodia: opening the account page")
    openAllodiaAccountPage(this, url)
}

// How long a hop runs before the first-run card owes the person a way back (docs/onboarding.md).
//
// Returning to the app is the ordinary way out on Android, and onResume already takes it, but
// only once there is a browser to have been dismissed. The window this covers is the one before
// that: the metadata read blocks on the network, and while it hangs the card spins over an app
// the person is already looking at, with nothing to come back from.
internal const val ALLODIA_SIGN_IN_ESCAPE_AFTER_MS = 8_000L

private fun MainActivity.beginAllodia(create: Boolean) {
    val activity = this
    val instance = activity.app ?: return
    if (activity.signingInAllodia) return
    activity.allodiaFailure = null
    activity.signingInAllodia = true
    activity.allodiaSignInSlow = false
    val attempt = activity.allodiaAttempt + 1
    activity.allodiaAttempt = attempt
    logUiInfo("allodia: sign-in starting")
    // The way back, armed rather than drawn: a hop that comes straight back never puts a button in
    // front of somebody who had no reason to read it.
    activity.mainHandler.postDelayed({
        if (activity.allodiaAttempt == attempt && activity.signingInAllodia) {
            activity.allodiaSignInSlow = true
        }
    }, ALLODIA_SIGN_IN_ESCAPE_AFTER_MS)
    thread(name = "mailcal-allodia-begin") {
        try {
            val start = if (create) {
                instance.beginAllodiaRegistration(AllodiaOAuthConfig.REDIRECT_URI)
            } else {
                instance.beginAllodiaSignIn(AllodiaOAuthConfig.REDIRECT_URI)
            }
            activity.mainHandler.post {
                // The read that just finished may be one the person gave up on. Opening a browser
                // for an attempt they escaped is the whole reason this is checked here.
                if (activity.allodiaAttempt != attempt) {
                    logUiInfo("allodia: sign-in was cancelled before the browser opened")
                    return@post
                }
                activity.pendingAllodiaSignIn = start.pending
                logUiInfo("allodia: opening the browser")
                openAllodiaSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("allodia: sign-in could not start (${e.message})")
            activity.mainHandler.post {
                if (activity.allodiaAttempt != attempt) return@post
                activity.signingInAllodia = false
                activity.allodiaSignInSlow = false
                activity.allodiaFailure = e.message.orEmpty()
            }
        }
    }
}

// The person's way out of a hop that did not come back. Nothing is reported: a sign-in somebody
// abandoned is not a failure, and the card returns to the offer it started from.
//
// The attempt is retired rather than only flagged, so a metadata read still running cannot open a
// browser afterwards, that is what the escape has to prevent, not merely stop drawing.
internal fun MainActivity.cancelAllodiaSignIn() {
    if (!signingInAllodia) return
    logUiInfo("allodia: sign-in cancelled by the person")
    allodiaAttempt += 1
    signingInAllodia = false
    allodiaSignInSlow = false
    pendingAllodiaSignIn = null
}

// Completes a sign-in from the redirect [callbackUrl]: the core exchanges the code, asks the
// service whose account it is, and stores the grant before it reports success. Nothing here writes
// a credential.
internal fun MainActivity.completeAllodiaSignIn(callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingAllodiaSignIn ?: run {
        // The redirect arrived with nothing armed to receive it, the sign-in was cancelled before
        // the browser came back. Silence here is what makes such a thing invisible, so say it.
        logUiWarn("allodia: redirect arrived but no sign-in was pending; ignoring it")
        return
    }
    activity.pendingAllodiaSignIn = null
    activity.signingInAllodia = true
    logUiInfo("allodia: redirect received, completing")
    thread(name = "mailcal-allodia-complete") {
        try {
            val account = instance.completeAllodiaSignIn(pending, callbackUrl)
            logUiInfo("allodia: signed in")
            activity.mainHandler.post {
                activity.allodiaAccount = account
                activity.allodiaFailure = null
                activity.signingInAllodia = false
                activity.allodiaSignInSlow = false
                // The first thing a new sign-in is for: this device's accounts go up, and whatever
                // the person's other devices hold comes back.
                activity.syncAllodiaAccounts()
            }
        } catch (e: Exception) {
            logUiWarn("allodia: sign-in did not complete (${e.message})")
            activity.mainHandler.post {
                activity.signingInAllodia = false
                activity.allodiaSignInSlow = false
                activity.allodiaFailure = e.message.orEmpty()
            }
        }
    }
}

// Signs out: the core forgets the account and erases its stored grant before it touches the
// network, then hands back where to end the browser's own session.
//
// Opening that page is best-effort and deliberately not reported: this device is already signed
// out whatever happens to it. What it buys is the next sign-in asking who you are, instead of
// completing silently against a session someone thought they had left. It does not end the grant
// at the service, a refresh token carrying offline_access outlives it by design.
//
// The account is forgotten in memory whatever the store does, so the card is re-read from the core
// rather than cleared here: a delete that failed leaves the app signed out and says why.
internal fun MainActivity.signOutOfAllodia() {
    val instance = app ?: return
    var endSession: String? = null
    allodiaFailure = try {
        endSession = instance.signOutOfAllodia()
        null
    } catch (e: Exception) {
        logUiWarn("allodia: sign-out did not complete (${e.message})")
        e.message.orEmpty()
    }
    allodiaAccount = instance.allodiaAccount()
    // Nothing left to say about other devices once this one is signed out of the account that
    // linked them.
    allodiaSync = AllodiaSyncState()
    endSession?.let { url ->
        logUiInfo("allodia: ending the browser session")
        openAllodiaAccountPage(this, url)
    }
}
