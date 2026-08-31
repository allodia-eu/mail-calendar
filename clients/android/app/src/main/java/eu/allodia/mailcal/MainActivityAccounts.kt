// Adding and removing a mail account, split out of MainActivityCore.kt: email-first detection, the
// shared add/remove path every setup route funnels through, and the Microsoft/Google/JMAP OAuth
// sign-in + re-authentication flows that end in it.
package eu.allodia.mailcal

import android.util.Log
import kotlin.concurrent.thread
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation
import uniffi.mailcal_bindings.beginGoogleLogin
import uniffi.mailcal_bindings.beginMicrosoftLogin

private const val TAG = "Mailcal"

// Connect an added account off the main thread, then persist its config under the core id.
// Detects a provider's settings from just the email address, off the main thread (the core
// call blocks up to ~10 s). The device's own DNS answers the MX fallback via AndroidMxResolver.
// Returns a manual fallback if the app somehow isn't up (the setup flow only shows when it is).
internal suspend fun MainActivity.detectAccount(email: String): SetupRecommendation {
    val instance = app ?: return SetupRecommendation.Manual(MissReason.NETWORK_ERROR)
    return withContext(Dispatchers.IO) {
        instance.detectAccountSettings(email, AndroidMxResolver(this@detectAccount))
    }
}

internal fun MainActivity.addAccount(configToml: String) {
    val activity = this
    val instance = activity.app ?: return
    activity.addError = null
    activity.isConnecting = true
    thread(name = "mailcal-add-account") {
        try {
            val row = instance.addAccount(configToml)
            instance.calendarConnectError()?.let {
                Log.w(TAG, "calendar (CalDAV) failed to connect: $it")
            }
            activity.mainHandler.post {
                activity.isConnecting = false
                activity.addingAccount = false
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
                activity.needsSetup = false
            }
        } catch (e: Exception) {
            Log.e(TAG, "add account failed: ${e.message}")
            activity.mainHandler.post {
                activity.isConnecting = false
                activity.addError = e.message ?: "unknown error"
            }
        }
    }
}

// Removes account [id] off the main thread: drops it from the running core (which stops its
// background sync, takes it out of the switcher/list, and, if it was selected, falls back to
// the unified inbox, then rebuilds the snapshot so the UI updates via the observer) and deletes
// its stored credential so it doesn't return on the next launch.
internal fun MainActivity.removeAccount(id: String) {
    val instance = app ?: return
    thread(name = "mailcal-remove-account") {
        try {
            instance.removeAccount(id)
        } catch (e: Exception) {
            // The account IS gone from the app, only erasing its stored credential failed, so it
            // would come back at the next launch. Say so rather than letting it reappear silently.
            logUiWarn("remove account: the credential could not be erased: ${e.message}")
        }
    }
}

// Starts the Microsoft sign-in: asks the core for the authorization URL, opens it in the
// browser (Custom Tabs), and holds the pending handle until the redirect returns to
// onNewIntent → completeMicrosoftLogin. Fast (no blocking), so it stays on the main thread.
internal fun MainActivity.signInWithMicrosoft(loginHint: String? = null) {
    if (app == null) return
    addError = null
    try {
        val start = beginMicrosoftLogin(
            MicrosoftOAuthConfig.TENANT,
            MicrosoftOAuthConfig.REDIRECT_URI,
            // The address the user is connecting (from autodetection), so Microsoft targets
            // that account instead of offering a different signed-in one. Null ⇒ the picker.
            loginHint?.takeIf { it.isNotBlank() },
        )
        pendingMicrosoftLogin = start.pending
        signingInMicrosoft = true
        openMicrosoftSignIn(this, start.authorizationUrl)
    } catch (e: Exception) {
        logUiWarn("microsoft sign-in start failed: ${e.message}")
        addError = e.message ?: "Microsoft sign-in failed to start"
    }
}

// Completes the Microsoft sign-in from the redirect [callbackUrl]: exchanges the code, connects
// the account, and stores its config, all off the main thread (the token exchange + folder
// connect block). The account appears at once; its first sync runs in the background.
internal fun MainActivity.completeMicrosoftLogin(callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingMicrosoftLogin ?: return
    activity.pendingMicrosoftLogin = null
    thread(name = "mailcal-ms-complete") {
        try {
            instance.completeMicrosoftLogin(pending, callbackUrl)
            activity.mainHandler.post {
                activity.signingInMicrosoft = false
                activity.addingAccount = false
                activity.needsSetup = false
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
                // A re-auth to grant calendar clears the account's re-consent flag in the core;
                // re-pull so the "reconnect for calendar" banner disappears at once.
                activity.refreshConnectivity()
            }
        } catch (e: Exception) {
            logUiWarn("microsoft complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInMicrosoft = false
                activity.addError = e.message ?: "Microsoft sign-in failed"
            }
        }
    }
}

// Starts the Google sign-in: asks the core for the authorization URL, opens it in the browser
// (Custom Tabs), and holds the pending handle until the redirect returns to onNewIntent →
// completeGoogleLogin. Fast (no blocking), so it stays on the main thread. The setup UI's Early
// Access confirmation gates the button that reaches here, Google hard-blocks anyone not on the
// app's OAuth test-user allow-list, so this is never reached without that acknowledgement.
internal fun MainActivity.signInWithGoogle(loginHint: String? = null) {
    if (app == null) return
    addError = null
    try {
        // Null only in a build carrying no Google registration, whose setup screen offers no
        // route here at all.
        val redirectUri = GoogleOAuthConfig.REDIRECT_URI ?: return
        val start = beginGoogleLogin(
            redirectUri = redirectUri,
            // The address the user is connecting (from autodetection), so Google targets that
            // account instead of offering a different signed-in one. Null ⇒ the picker.
            loginHint = loginHint?.takeIf { it.isNotBlank() },
        )
        pendingGoogleLogin = start.pending
        signingInGoogle = true
        openGoogleSignIn(this, start.authorizationUrl)
    } catch (e: Exception) {
        logUiWarn("google sign-in start failed: ${e.message}")
        addError = e.message ?: "Google sign-in failed to start"
    }
}

// Completes the Google sign-in from the redirect [callbackUrl]: exchanges the code, connects the
// account, and stores its config, all off the main thread (the token exchange + connect block).
// The account appears at once; its first sync runs in the background.
internal fun MainActivity.completeGoogleLogin(callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingGoogleLogin ?: return
    activity.pendingGoogleLogin = null
    thread(name = "mailcal-google-complete") {
        try {
            instance.completeGoogleLogin(pending, callbackUrl)
            activity.mainHandler.post {
                activity.signingInGoogle = false
                activity.addingAccount = false
                activity.needsSetup = false
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
            }
        } catch (e: Exception) {
            logUiWarn("google complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInGoogle = false
                activity.addError = e.message ?: "Google sign-in failed"
            }
        }
    }
}

// Whether this JMAP server offers discoverable OAuth sign-in, so the setup form knows whether to
// show the button at all. Blocking (it makes network round trips), always call from a background
// thread, never from composition. Never throws: any failure is simply "no".
internal fun MainActivity.jmapSignInAvailable(email: String, serverUrl: String?): Boolean {
    val instance = app ?: return false
    return try {
        instance.jmapOauthAvailable(email, serverUrl?.takeIf { it.isNotBlank() })
    } catch (e: Exception) {
        // Defensive only, the FFI is declared non-throwing. A server that doesn't do this is the
        // normal case, not an error worth showing anyone.
        logUiInfo("jmap oauth: availability check could not run (${e.message}); offering the manual secret only")
        false
    }
}

// Starts a JMAP sign-in: asks the core for the authorization URL, which runs the whole discovery
// + dynamic-registration chain, so this BLOCKS on the network and must run off the main thread:
// then opens the browser and holds the pending handle until the redirect returns to onNewIntent →
// completeJmapSignIn.
//
// Unlike Microsoft/Google there is no client id to pass: the core registers this install with the
// discovered server itself (RFC 7591). A failure here is expected on servers that don't support
// this, so it surfaces as the "use a password or API token instead" note rather than a hard error.
internal fun MainActivity.signInWithJmap(email: String, serverUrl: String?) {
    val activity = this
    val instance = activity.app ?: return
    activity.addError = null
    activity.signingInJmap = true
    logUiInfo("jmap sign-in: starting (server hint: ${serverUrl?.ifBlank { null } ?: "derived from the email domain"})")
    thread(name = "mailcal-jmap-begin") {
        try {
            val start = instance.beginJmapLogin(
                email = email,
                serverUrl = serverUrl?.takeIf { it.isNotBlank() },
                redirectUri = JmapOAuthConfig.REDIRECT_URI,
            )
            activity.mainHandler.post {
                activity.pendingJmapLogin = start.pending
                // Both JMAP flows come back through one redirect scheme, and a re-authentication
                // the user ABANDONED in the browser leaves its account marker set, after which
                // this redirect would be routed to completeJmapReauth with a stale account and
                // this flow's pending handle. Claim the slot for a new account explicitly.
                activity.pendingJmapReauthAccount = null
                logUiInfo("jmap sign-in: opening the browser")
                openJmapSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("jmap sign-in start failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInJmap = false
                // Deliberately the friendly fallback line, not the raw cause: the manual secret
                // field is still right there and still works. The cause is in the file log.
                activity.addError = L10n.setup_jmap_signin_failed(activity)
            }
        }
    }
}

// Completes a JMAP sign-in from the redirect [callbackUrl]: exchanges the code for tokens, then
// hands the resulting config TOML to the SAME addAccount + SecureStore path the manual JMAP form
// uses, the core deliberately returns the identical `[jmap]` shape, so there is no second
// storage path to keep in step. Off the main thread (the exchange and connect both block).
internal fun MainActivity.completeJmapSignIn(callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingJmapLogin ?: run {
        // The redirect arrived with nothing armed to receive it, the sign-in was cancelled (or
        // cleared) before the browser came back. Silence here is what made this invisible the
        // first time it happened, so say so.
        logUiWarn("jmap sign-in: redirect arrived but no sign-in was pending; ignoring it")
        return
    }
    activity.pendingJmapLogin = null
    // Keep the card busy from here until the account is added, the exchange plus the connect is
    // a couple of seconds of real work, and it should look like it.
    activity.completingJmap = true
    activity.signingInJmap = true
    logUiInfo("jmap sign-in: redirect received, completing")
    thread(name = "mailcal-jmap-complete") {
        try {
            val configToml = instance.completeJmapLogin(pending, callbackUrl)
            logUiInfo("jmap sign-in: token exchange succeeded; adding the account")
            activity.mainHandler.post {
                activity.completingJmap = false
                activity.signingInJmap = false
                // addAccount owns the connect + persist-after-connect discipline, and reports its
                // own failure through addError.
                activity.addAccount(configToml)
            }
        } catch (e: Exception) {
            logUiWarn("jmap sign-in complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.completingJmap = false
                activity.signingInJmap = false
                activity.addError = L10n.setup_jmap_signin_failed(activity)
            }
        }
    }
}

// Signs an existing OAuth JMAP account back in, from the expired-sign-in banner. Unlike
// signInWithJmap this makes no discovery calls: the core builds the authorization URL from that
// account's own persisted grant, so nothing is re-registered and the same client id is replayed.
// It still runs off the main thread, because the two paths are otherwise identical from here.
internal fun MainActivity.reconnectJmap(accountId: String) {
    val activity = this
    val instance = activity.app ?: return
    if (activity.signingInJmap) return
    activity.signingInJmap = true
    logUiInfo("jmap sign-in: re-authenticating an existing account")
    thread(name = "mailcal-jmap-reauth-begin") {
        try {
            val start = instance.beginJmapReauth(accountId)
            activity.mainHandler.post {
                activity.pendingJmapLogin = start.pending
                // What routes the redirect to completeJmapReauth instead of a new-account add.
                activity.pendingJmapReauthAccount = accountId
                logUiInfo("jmap sign-in: opening the browser to re-authenticate")
                openJmapSignIn(activity, start.authorizationUrl)
            }
        } catch (e: Exception) {
            logUiWarn("jmap re-authentication start failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInJmap = false
                activity.toastSignInFailed()
            }
        }
    }
}

// Completes a JMAP RE-authentication from the redirect [callbackUrl]. There is no addAccount here:
// [accountId] already exists, so the core connects with the fresh grant, writes it to the secure
// store through the same port a token rotation uses, and retracts the expired-sign-in prompt:
// which is what makes the banner disappear. Off the main thread (the exchange and connect block).
internal fun MainActivity.completeJmapReauth(accountId: String, callbackUrl: String) {
    val activity = this
    val instance = activity.app ?: return
    val pending = activity.pendingJmapLogin ?: run {
        logUiWarn("jmap sign-in: redirect arrived but no re-authentication was pending; ignoring it")
        return
    }
    activity.pendingJmapLogin = null
    activity.pendingJmapReauthAccount = null
    activity.completingJmap = true
    logUiInfo("jmap sign-in: redirect received, completing the re-authentication")
    thread(name = "mailcal-jmap-reauth-complete") {
        try {
            instance.completeJmapReauth(accountId, pending, callbackUrl)
            logUiInfo("jmap sign-in: the account is connected again")
            activity.mainHandler.post {
                activity.completingJmap = false
                activity.signingInJmap = false
            }
        } catch (e: Exception) {
            logUiWarn("jmap re-authentication complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.completingJmap = false
                activity.signingInJmap = false
                activity.toastSignInFailed()
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

// Called from onResume: resets any sign-in whose browser can close with no callback (Custom Tabs
// give none) and so leaves no way for its spinner to hear the flow died. JMAP is the exception,
// its tab runs in its OWN task (openJmapSignIn), so resuming does not mean the browser is gone,
// and Allodia's spinner is reset even though its flow lives in MainActivityAllodia.kt: it shares
// the same Custom-Tab-in-this-task shape as Microsoft/Google, so it belongs with this cleanup
// rather than splitting one lifecycle concern across two files.
internal fun MainActivity.cancelAbandonedSignIns() {
    // Returning from the Microsoft browser with the sign-in still pending means the user
    // dismissed it without completing (a real redirect nulls the pending handle in onNewIntent
    // before we get here). Reset the spinner so the button doesn't spin forever.
    if (signingInMicrosoft && pendingMicrosoftLogin != null) {
        logUiInfo("microsoft sign-in cancelled (browser dismissed)")
        signingInMicrosoft = false
        pendingMicrosoftLogin = null
    }
    // Same for a dismissed Google browser sign-in.
    if (signingInGoogle && pendingGoogleLogin != null) {
        logUiInfo("google sign-in cancelled (browser dismissed)")
        signingInGoogle = false
        pendingGoogleLogin = null
    }
    // And for a dismissed Allodia account sign-in. Its Custom Tab runs in THIS task (the
    // Microsoft/Google shape, not JMAP's own-task one), so resuming means the tab is gone:
    // and without this the Settings card would sit on "Signing in…" with no way back.
    if (signingInAllodia && pendingAllodiaSignIn != null) {
        logUiInfo("allodia: sign-in cancelled (browser dismissed)")
        signingInAllodia = false
        allodiaSignInSlow = false
        pendingAllodiaSignIn = null
    }
    // A JMAP sign-in is handled differently on purpose. Its browser runs in its own task
    // (see openJmapSignIn), so resuming MainActivity does NOT mean the browser is gone, the
    // user may simply have stepped out to a password manager and come back. So the spinner is
    // cleared, because a form that looks busy while the user is staring at it is a lie, but
    // the pending handle is deliberately KEPT: if they return to the tab and finish, the
    // redirect must still complete. Clearing it here is what silently broke the flow the
    // first time someone reached for their password.
    //
    // `completingJmap` is the exception: when the redirect has already landed we ARE busy, and
    // onNewIntent runs just before this, so clearing the spinner here would immediately undo
    // the one it just set.
    if (signingInJmap && !completingJmap) {
        logUiInfo("jmap sign-in: app resumed while a sign-in is in flight; keeping it armed")
        signingInJmap = false
    }
}
