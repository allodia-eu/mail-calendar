// The "already set up" refusal's offer to put the credential just given on the account already
// there (docs/accounts.md rule 17), asking first when that sign-in will not reach the account's
// calendar or contacts (rule 18). The update blocks on a connect, so it runs off the main thread;
// the core stores the credential and raises the Settings signal.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.UnreachedUses

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
            existing.nextcloudGrant?.let { instance.completeAccountNextcloudSignIn(existing.accountId, it) }
                ?: instance.useCredentialFrom(existing.accountId, existing.configToml, unreached)
        }
        mainHandler.post {
            setupFlow.updatingExisting = false
            setupFlow.existing = null
            // The core has the grant now, or withdrew it as somebody else's.
            if (answer.isSuccess || answer.exceptionOrNull() is MailcalException.DifferentAccount) {
                setupFlow.nextcloud.take()
            }
            answer.onSuccess {
                logUiInfo("setup: the account already set up took the new sign-in")
                refreshAccountsSnapshot()
                pullNotices()
                endSetupAfterUpdate()
            }
            answer.onFailure {
                logUiWarn("setup: updating the account already set up failed")
                reportAddFailure(it)
            }
        }
    }
}
