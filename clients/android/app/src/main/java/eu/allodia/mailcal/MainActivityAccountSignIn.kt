// How an IMAP or JMAP account signs in, changed or renewed from its page in Settings or from the
// expired-sign-in banner, and the "already set up" refusal's offer to update the account already
// there (docs/accounts.md rule 17). Either asks first when the new sign-in will not reach the
// account's calendar or contacts (rule 18). Every core call here blocks (detection, discovery, a token
// exchange, a connect), so each runs off the main thread. The core stores the credential and
// raises the Settings signal; the page redraws from the next snapshot.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.UnreachedUses

/** Asks account [accountId]'s server which ways in it takes, once per opening of its page. */
internal fun MainActivity.askSignInOptions(accountId: String) {
    val instance = app ?: return
    val signIn = accountSettings.signIn
    if (!AccountSignInRules.switchable(accountSettings.entry(accountId)?.kind)) return
    if (!signIn.startAsking(accountId)) return
    thread(name = "mailcal-sign-in-options") {
        // Never throws, and bounded: an unanswered question offers nothing.
        val options = instance.accountSignInOptions(accountId)
        mainHandler.post { signIn.answered(accountId, options) }
    }
}

/** The title a failed sign-in is said under: a change of method, or a sign-in again. */
private fun MainActivity.signInFailedTitle(switching: Boolean): String =
    if (switching) L10n.settings_account_switch_failed(this) else L10n.settings_account_signin_failed(this)

/**
 * Signs account [accountId] in through its provider in the browser: again, from its stored grant,
 * or for the first time in place of its password when [switching]. A switch that will not reach
 * the account's calendar or contacts asks first, and the browser waits for [answerUnreached]. The
 * redirect comes back through onNewIntent to [completeAccountSignIn].
 */
internal fun MainActivity.startAccountSignIn(accountId: String, switching: Boolean) {
    val instance = app ?: return
    val signIn = accountSettings.signIn
    val redirect = AccountSignInRules.redirectFor(accountSettings.entry(accountId)?.kind) ?: return
    if (!signIn.begin(accountId, switching)) return
    thread(name = "mailcal-account-sign-in-begin") {
        val start = runCatching { instance.beginAccountSignIn(accountId, redirect) }
        mainHandler.post {
            start.onSuccess {
                val set = UnreachedRules.setOf(it.unreached)
                if (set != null) {
                    signIn.ask(UnreachedQuestion(accountId, set, it.pending, it.authorizationUrl, redirect))
                    return@onSuccess
                }
                signIn.opened(it.pending)
                openAccountBrowser(it.authorizationUrl, redirect, switching)
            }
            start.onFailure {
                logUiWarn("sign-in: starting an account's sign-in failed: ${coreErrorText(it)}")
                signIn.finished()
                say(
                    AccountNotice(
                        NoticeKind.FAILURE,
                        signInFailedTitle(switching),
                        coreErrorSaid(it, this) ?: L10n.account_signin_browser_failed(this),
                    ),
                )
            }
        }
    }
}

/** Opens the browser for the sign-in [AccountSignInState.opened] armed. */
private fun MainActivity.openAccountBrowser(url: String, redirect: String, switching: Boolean) {
    val signIn = accountSettings.signIn
    val opened = runCatching {
        if (redirect == ImapOAuthConfig.REDIRECT_URI) openImapSignIn(this, url) else openJmapSignIn(this, url)
    }
    opened.onFailure { error ->
        logUiWarn("sign-in: the browser did not open: ${error.message}")
        signIn.yieldRedirect()
        signIn.finished()
        val title = signInFailedTitle(switching)
        say(AccountNotice(NoticeKind.FAILURE, title, L10n.account_signin_browser_failed(this)))
    }
}

/** The switch's question answered: on to the browser with the answer, or nothing changes. */
internal fun MainActivity.answerUnreached(choice: UnreachedChoice) {
    val switching = accountSettings.signIn.running?.switching ?: true
    val asked = accountSettings.signIn.decided(UnreachedRules.answer(choice)) ?: return
    logUiInfo("sign-in: what the new sign-in does not reach was answered")
    openAccountBrowser(asked.authorizationUrl, asked.redirect, switching)
}

/** Whether the next IMAP or JMAP redirect completes an existing account's sign-in. */
internal fun MainActivity.awaitsAccountSignIn(): Boolean = accountSettings.signIn.pending != null

/** Completes [startAccountSignIn] from the redirect: the exchange and the connect block. */
internal fun MainActivity.completeAccountSignIn(callbackUrl: String) {
    val instance = app ?: return
    val signIn = accountSettings.signIn
    val claimed = signIn.claim() ?: return
    thread(name = "mailcal-account-sign-in-complete") {
        val answer = runCatching {
            instance.completeAccountSignIn(claimed.accountId, claimed.handle, callbackUrl, claimed.unreached)
        }
        mainHandler.post {
            signIn.finished(switched = answer.isSuccess && claimed.switching)
            refreshAccountsSnapshot()
            refreshConnectivity()
            answer.onSuccess {
                logUiInfo("sign-in: an account signed in through its provider")
                val said = if (claimed.switching) {
                    L10n.settings_account_switched_to_provider(this)
                } else {
                    L10n.settings_account_signed_in(this)
                }
                say(AccountNotice(NoticeKind.SUCCESS, said, ""))
                if (claimed.switching) askSignInOptions(claimed.accountId)
            }
            answer.onFailure {
                logUiWarn("sign-in: completing an account's sign-in failed: ${coreErrorText(it)}")
                say(
                    AccountNotice(
                        NoticeKind.FAILURE,
                        signInFailedTitle(claimed.switching),
                        coreErrorSaid(it, this) ?: L10n.error_unknown(this),
                    ),
                )
            }
        }
    }
}

/** Switches account [accountId] to signing in with [password], which the core tests first. */
internal fun MainActivity.switchToPassword(accountId: String, password: String) {
    val instance = app ?: return
    val signIn = accountSettings.signIn
    if (!signIn.beginPassword(accountId)) return
    thread(name = "mailcal-account-to-password") {
        val answer = runCatching { instance.switchAccountToPassword(accountId, password) }
        mainHandler.post {
            signIn.finished(switched = answer.isSuccess)
            refreshAccountsSnapshot()
            answer.onSuccess {
                logUiInfo("sign-in: an account now signs in with a password")
                say(AccountNotice(NoticeKind.SUCCESS, L10n.settings_account_switched_to_password(this), ""))
                refreshConnectivity()
                askSignInOptions(accountId)
            }
            answer.onFailure {
                logUiWarn("sign-in: switching an account to a password failed: ${coreErrorText(it)}")
                say(
                    AccountNotice(
                        NoticeKind.FAILURE,
                        L10n.settings_account_switch_failed(this),
                        coreErrorSaid(it, this) ?: L10n.error_unknown(this),
                    ),
                )
            }
        }
    }
}

/** A sign-in from the setup form or the banner takes the IMAP and JMAP redirects for itself. */
internal fun MainActivity.yieldAccountSignIn() = accountSettings.signIn.yieldRedirect()

/**
 * The add was refused because the account is already set up: offers to put the credential of
 * [configToml] on it, when Settings can switch that account's sign-in.
 */
internal fun MainActivity.offerExistingAccount(
    error: Throwable,
    configToml: String,
    unreached: List<AccountCapability> = emptyList(),
) {
    setupFlow.existing = AccountSignInRules.existingAccount(error, configToml) {
        accountSettings.entry(it)?.kind
    }?.copy(unreached = unreached)
}

/**
 * "Update the existing account": asks first when the sign-in the config came from will not reach
 * the calendar or contacts, and updates at once otherwise.
 */
internal fun MainActivity.pressUpdateExisting() {
    val existing = setupFlow.existing ?: return
    if (UnreachedRules.asks(existing.unreached)) {
        setupFlow.askingExisting = true
    } else {
        updateExistingAccount(null)
    }
}

/** The update's question answered; Cancel leaves the refusal on screen. */
internal fun MainActivity.answerExistingUnreached(choice: UnreachedChoice) {
    setupFlow.askingExisting = false
    UnreachedRules.answer(choice)?.let { updateExistingAccount(it) }
}

/**
 * Puts the credential of the refused add on the account already there, doing with what it does
 * not reach what [unreached] says. That account is listed already, so setup ends as an add does
 * without adding anything; a failure is said on the form as an add's is.
 */
private fun MainActivity.updateExistingAccount(unreached: UnreachedUses?) {
    val instance = app ?: return
    val existing = setupFlow.existing ?: return
    if (setupFlow.updatingExisting) return
    setupFlow.updatingExisting = true
    thread(name = "mailcal-update-existing") {
        val answer = runCatching {
            instance.useCredentialFrom(existing.accountId, existing.configToml, unreached)
        }
        mainHandler.post {
            setupFlow.updatingExisting = false
            setupFlow.existing = null
            answer.onSuccess {
                logUiInfo("setup: the account already set up took the new sign-in")
                refreshAccountsSnapshot()
                refreshConnectivity()
                endSetupAfterUpdate()
            }
            answer.onFailure {
                logUiWarn("setup: updating the account already set up failed")
                reportAddFailure(it)
            }
        }
    }
}
