// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18), from setup's
// step after a sign-in that did not reach them and from the account page. The connect blocks (two
// connects), and Skip's switching off deletes, so both run off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.CapabilityChange

/**
 * Moves the uses [state]'s form names to an account of their own signing in as [username] with
 * [password], then goes on as [separateLinked] says; a failure is said under the fields.
 */
private fun MainActivity.connectSeparate(state: SeparateSignIn, username: String, password: String) {
    val instance = app ?: return
    val form = state.form ?: return
    if (!state.start()) return
    thread(name = "mailcal-separate-server") {
        val answer = runCatching {
            instance.addLinkedCalendarAccount(form.accountId, form.uses, username, password)
        }
        mainHandler.post {
            answer.onSuccess {
                logUiInfo("accounts: a calendar or address book now signs in on its own")
                separateLinked(state)
            }
            answer.onFailure {
                logUiWarn("accounts: a calendar or address book did not connect on its own: ${coreErrorText(it)}")
                state.failed(coreErrorSaid(it, this) ?: L10n.error_unknown(this))
            }
        }
    }
}

/** The uses moved to an account of their own, by a password or a sign-in: the lists redraw. */
internal fun MainActivity.separateAccountAdded() {
    refreshAccountsSnapshot()
    refreshConnectivity()
    // The account list changed, so the person's other devices should hear about it.
    readAccountsSynced()
    syncAllodiaAccounts()
}

/**
 * Where a form goes once its uses have an account of their own, however they got it: setup's step
 * on to the link step, whose pickers already show the new link, and the page's dialog closed, the
 * page redrawing from the next snapshot.
 */
internal fun MainActivity.separateLinked(state: SeparateSignIn) {
    separateAccountAdded()
    if (state !== setupFlow.separate) {
        state.close()
        return
    }
    if (!setupFlow.separateDone(::linkStepFor)) finishSetup()
}

/** Setup's step: Connect. */
internal fun MainActivity.connectSetupSeparate(username: String, password: String) {
    connectSeparate(setupFlow.separate, username, password)
}

/** Setup's step: Skip switches off exactly the uses the sign-in did not reach, then goes on. */
internal fun MainActivity.skipSetupSeparate() {
    val state = setupFlow.separate
    val form = state.form ?: return
    val instance = app
    if (instance == null) {
        if (!setupFlow.separateDone(::linkStepFor)) finishSetup()
        return
    }
    if (!state.start()) return
    thread(name = "mailcal-separate-skip") {
        UnreachedRules.skipChanges(form.uses).forEach { (capability, on) ->
            runCatching { instance.setAccountCapability(form.accountId, capability, on) }
                .onSuccess { if (it != CapabilityChange.APPLIED) logUiWarn("setup: a use was not switched off: $it") }
                .onFailure { logUiWarn("setup: a use could not be switched off: ${coreErrorText(it)}") }
        }
        mainHandler.post {
            refreshAccountsSnapshot()
            if (!setupFlow.separateDone(::linkStepFor)) finishSetup()
        }
    }
}

/** The account page's action: the form for every use of [accountId] that needs this. */
internal fun MainActivity.openPageSeparate(accountId: String) {
    val entry = accountSettings.entry(accountId) ?: return
    UnreachedRules.form(entry)?.let { accountSettings.separate.open(it) }
}

/** The account page's Connect. */
internal fun MainActivity.connectPageSeparate(username: String, password: String) {
    connectSeparate(accountSettings.separate, username, password)
}
