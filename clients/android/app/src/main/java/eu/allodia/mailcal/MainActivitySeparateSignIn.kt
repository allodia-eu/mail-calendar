// A separate calendar or address book server signed in to at its own provider (docs/mail-oauth.md
// rule 13), from the same form as its password, in setup's step and on the account page. The core
// asks the server who signs it in, registers with that issuer, exchanges the code and adds the
// account; this host asks whether to offer it, owns the browser hop and holds the pending handle in
// the form's SeparateServerSignInSlot. Every core call here blocks, so each runs off the main thread.
//
// The redirect comes back on the IMAP sign-in's own redirect, because that is the redirect this
// app registers with an issuer it discovers.
package eu.allodia.mailcal

import android.content.Context
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.MailcalException

/** What a sign-in that did not start or complete says under the form. */
internal fun separateSignInSaid(error: Throwable, ctx: Context): String = when (error) {
    is MailcalException.Connect -> L10n.setup_imap_signin_failed(ctx)
    else -> coreErrorSaid(error, ctx) ?: L10n.setup_imap_signin_failed(ctx)
}

/** Asks, once the form is drawn, whether its server signs in at its own provider. */
internal fun MainActivity.askSeparateSignIn(state: SeparateSignIn) {
    val instance = app ?: return
    val form = state.form ?: return
    if (!state.startAsking(form)) return
    thread(name = "mailcal-separate-offer") {
        // Never an error from the core: a server that cannot be asked offers the password.
        val offers = runCatching { instance.separateServerOffersSignIn(form.accountId, form.uses) }
            .getOrDefault(false)
        mainHandler.post { state.answered(form, offers) }
    }
}

/**
 * Starts the sign-in for [state]'s form, superseding one still out, and opens the browser unless
 * the attempt was superseded or the form left meanwhile.
 */
internal fun MainActivity.signInSeparate(state: SeparateSignIn) {
    val instance = app ?: return
    val form = state.form ?: return
    val attempt = state.beginSignIn() ?: return
    logUiInfo("accounts: signing a calendar or address book in at its own provider")
    thread(name = "mailcal-separate-sign-in-begin") {
        val start = runCatching {
            instance.beginSeparateServerSignIn(form.accountId, form.uses, ImapOAuthConfig.REDIRECT_URI)
        }
        mainHandler.post {
            start.onSuccess {
                if (!state.browser.opening(attempt, it.pending)) {
                    logUiInfo("accounts: the form was left before the browser opened")
                    return@onSuccess
                }
                claimImapRedirect(state)
                runCatching { openImapSignIn(this, it.authorizationUrl) }.onFailure { error ->
                    logUiWarn("accounts: the browser did not open: ${error.message}")
                    state.browser.retire()
                    state.failed(L10n.setup_imap_signin_failed(this))
                }
            }
            start.onFailure {
                if (!state.browser.settled(attempt)) return@onFailure
                logUiWarn("accounts: a separate server's sign-in did not start: ${coreErrorText(it)}")
                state.failed(separateSignInSaid(it, this))
            }
        }
    }
}

/**
 * The redirect goes to the sign-in that opened its browser last. This one just did, so the IMAP
 * sign-ins that share its redirect stop waiting for it.
 */
private fun MainActivity.claimImapRedirect(state: SeparateSignIn) {
    yieldAccountSignIn()
    if (pendingImapLogin != null) {
        pendingImapLogin = null
        signingInImap = false
    }
    separateForms().filter { it !== state && it.browser.pending != null }.forEach { it.browser.retire() }
}

private fun MainActivity.separateForms(): List<SeparateSignIn> = listOf(setupFlow.separate, accountSettings.separate)

/** The form whose sign-in the next IMAP redirect completes, or null when none waits for one. */
internal fun MainActivity.awaitingSeparateSignIn(): SeparateSignIn? =
    separateForms().firstOrNull { it.browser.pending != null }

/** Completes [state]'s sign-in from the redirect, and goes on as Connect's success does. */
internal fun MainActivity.completeSeparateSignIn(state: SeparateSignIn, callbackUrl: String) {
    val instance = app ?: return
    val form = state.form ?: return
    val (attempt, pending) = state.browser.redirected() ?: return
    logUiInfo("accounts: a separate server's sign-in came back; completing it")
    thread(name = "mailcal-separate-sign-in-complete") {
        val answer = runCatching { instance.completeSeparateServerSignIn(form.accountId, pending, callbackUrl) }
        mainHandler.post {
            if (!state.browser.settled(attempt)) {
                // The core added the account before the form moved on, so the lists still redraw.
                if (answer.isSuccess) separateAccountAdded()
                logUiInfo("accounts: the form moved on while a sign-in completed")
                return@post
            }
            answer.onSuccess {
                logUiInfo("accounts: a calendar or address book now signs in at its own provider")
                separateLinked(state)
            }
            answer.onFailure {
                logUiWarn("accounts: a separate server's sign-in did not complete: ${coreErrorText(it)}")
                state.failed(separateSignInSaid(it, this))
            }
        }
    }
}

/**
 * The app is in front again. A separate server's tab runs in a task of its own, as the IMAP
 * sign-in's does, so the spinner goes and the handle stays armed for a redirect still to come.
 */
internal fun MainActivity.resumeSeparateSignIns() {
    separateForms().forEach {
        if (it.browser.resumed()) logUiInfo("accounts: back from a separate server's sign-in; keeping it armed")
    }
}
