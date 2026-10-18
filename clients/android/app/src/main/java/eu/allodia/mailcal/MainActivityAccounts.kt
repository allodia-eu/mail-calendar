// Adding a mail account, split out of MainActivityCore.kt: email-first detection, the
// shared add path every setup route funnels through, and the Microsoft and Google OAuth sign-ins
// that end in it. JMAP's are MainActivityJmapSignIn.kt, IMAP's MainActivityImapSignIn.kt.
package eu.allodia.mailcal

import android.util.Log
import kotlin.concurrent.thread
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.SetupRecommendation
import uniffi.mailcal_bindings.beginGoogleLogin
import uniffi.mailcal_bindings.beginMicrosoftLogin

private const val TAG = "Mailcal"

// Connect an added account off the main thread, then persist its config under the core id.
// Detects a provider's settings from just the email address, off the main thread (the core
// call blocks up to ~10 s). The device's own DNS answers the MX fallback via AndroidMxResolver.
// Returns a manual fallback if the app somehow isn't up (the setup flow only shows when it is).
internal suspend fun MainActivity.detectAccount(email: String): DetectedSetup {
    val instance = app ?: return detected(SetupRecommendation.Manual(MissReason.NETWORK_ERROR))
    return withContext(Dispatchers.IO) {
        instance.detectAccountSetup(email, AndroidMxResolver(this@detectAccount))
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
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
                // Its uses, its link step, or the end of setup (MainActivitySetupFlow.kt).
                activity.afterAccountAdded(row.id)
            }
        } catch (e: Exception) {
            // The reason, never the certificate: a refusal's message carries the whole of it
            // (docs/certificate-exceptions.md rule 9).
            Log.e(TAG, "add account failed")
            val failure = (e as? MailcalException)?.let { connectFailure(it, activity) }
            activity.mainHandler.post {
                activity.isConnecting = false
                activity.addFailure = failure
                // A refusal the form can offer to accept is drawn as the panel and not said
                // twice, so only what the panel cannot carry is left as a message (rule 7).
                activity.addError = when {
                    failure?.certificate != null -> null
                    else -> coreErrorSaid(e, activity) ?: L10n.error_unknown(activity)
                }
            }
        }
    }
}

// Starts the Microsoft sign-in: asks the core for the authorization URL, opens it in the
// browser (Custom Tabs), and holds the pending handle until the redirect returns to
// onNewIntent → completeMicrosoftLogin. Fast (no blocking), so it stays on the main thread.
internal fun MainActivity.signInWithMicrosoft(
    loginHint: String? = null,
    capabilities: List<AccountCapability>? = null,
) {
    if (app == null) return
    addError = null
    try {
        val start = beginMicrosoftLogin(
            MicrosoftOAuthConfig.TENANT,
            MicrosoftOAuthConfig.REDIRECT_URI,
            // The address the user is connecting (from autodetection), so Microsoft targets
            // that account instead of offering a different signed-in one. Null ⇒ the picker.
            loginHint?.takeIf { it.isNotBlank() },
            // What the account is to be used for, so the provider is asked for that alone; null
            // asks for everything (docs/accounts.md rule 10).
            capabilities,
        )
        pendingMicrosoftLogin = start.pending
        // A new account, not one being signed in again: this redirect is the setup form's.
        accountSettings.yieldConsent()
        signingInMicrosoft = true
        openMicrosoftSignIn(this, start.authorizationUrl)
    } catch (e: Exception) {
        logUiWarn("microsoft sign-in start failed: ${e.message}")
        addError = coreErrorSaid(e, this) ?: "Microsoft sign-in failed to start"
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
            val row = instance.completeMicrosoftLogin(pending, callbackUrl)
            activity.mainHandler.post {
                activity.signingInMicrosoft = false
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
                activity.refreshConnectivity()
                activity.afterAccountAdded(row.id, signedInAtProvider = true)
            }
        } catch (e: Exception) {
            logUiWarn("microsoft complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInMicrosoft = false
                activity.addError = coreErrorSaid(e, activity) ?: "Microsoft sign-in failed"
            }
        }
    }
}

// Starts the Google sign-in: asks the core for the authorization URL, opens it in the browser
// (Custom Tabs), and holds the pending handle until the redirect returns to onNewIntent →
// completeGoogleLogin. Fast (no blocking), so it stays on the main thread. The setup UI's Early
// Access confirmation gates the button that reaches here, Google hard-blocks anyone not on the
// app's OAuth test-user allow-list, so this is never reached without that acknowledgement.
internal fun MainActivity.signInWithGoogle(
    loginHint: String? = null,
    capabilities: List<AccountCapability>? = null,
) {
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
            capabilities = capabilities,
        )
        pendingGoogleLogin = start.pending
        accountSettings.yieldConsent()
        signingInGoogle = true
        openGoogleSignIn(this, start.authorizationUrl)
    } catch (e: Exception) {
        logUiWarn("google sign-in start failed: ${e.message}")
        addError = coreErrorSaid(e, this) ?: "Google sign-in failed to start"
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
            val row = instance.completeGoogleLogin(pending, callbackUrl)
            activity.mainHandler.post {
                activity.signingInGoogle = false
                activity.addError = null
                // The account list changed, so the person's other devices should hear about
                // it now rather than at the next launch. A no-op when nobody is signed in.
                activity.readAccountsSynced()
                activity.syncAllodiaAccounts()
                activity.afterAccountAdded(row.id, signedInAtProvider = true)
            }
        } catch (e: Exception) {
            logUiWarn("google complete failed: ${e.message}")
            activity.mainHandler.post {
                activity.signingInGoogle = false
                activity.addError = coreErrorSaid(e, activity) ?: "Google sign-in failed"
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

// Called from onResume: resets any sign-in whose browser can close with no callback (Custom Tabs
// give none) and so leaves no way for its spinner to hear the flow died. JMAP and IMAP are the
// exception, their tabs run in their OWN task, so resuming does not mean the browser is gone,
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
    // And for a dismissed sign-in of an existing account (MainActivityAccountSettings.kt).
    cancelAbandonedConsent()
    // A JMAP or IMAP sign-in is handled differently on purpose. Its browser runs in its own task
    // (openJmapSignIn, openImapSignIn), so resuming MainActivity does NOT mean the browser is
    // gone: the person may simply have stepped out to a password manager and come back. So the
    // spinner is cleared, because a form that looks busy while somebody is looking at it is a
    // lie, but the pending handle is KEPT: if they return to the tab and finish, the redirect
    // must still complete. A redirect that already landed took the handle in onNewIntent, just
    // before this, and its exchange keeps the spinner (SignInSlot.resumed).
    if (jmapSignIn.resumed()) {
        logUiInfo("jmap sign-in: app resumed while a sign-in is in flight; keeping it armed")
    }
    if (imapSignIn.resumed()) {
        logUiInfo("imap sign-in: app resumed while a sign-in is in flight; keeping it armed")
    }
}
