// A calendar or address book given a sign-in of its own (docs/accounts.md rule 18), from setup's
// step after a sign-in that did not reach them and from the account page. Connect and Skip are
// changes on the account's page, which the core runs off this thread and draws on its section;
// the form ends once the core draws none.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.TypedPassword

/** The uses moved to an account of their own, or were switched off: the lists redraw. */
internal fun MainActivity.separateAccountAdded() {
    refreshAccountsSnapshot()
    pullNotices()
    // The account list changed, so the person's other devices should hear about it.
    readAccountsSynced()
    syncAllodiaAccounts()
}

/**
 * Where a form goes once nothing needs it, however that came about: setup's step on to the link
 * step, whose pickers already show the new link, and the page's dialog closed.
 */
internal fun MainActivity.separateLinked(state: SeparateSignIn) {
    // First, so the link step is drawn from the accounts as they are now.
    separateAccountAdded()
    if (state !== setupFlow.separate) {
        state.close()
        return
    }
    if (!setupFlow.separateDone(::linkStepFor)) finishSetup()
}

/** Draws each open form again from the core, after a signal that may have moved it. */
internal fun MainActivity.refreshSeparate() {
    val instance = app ?: return
    listOf(setupFlow.separate, accountSettings.separate).forEach { state ->
        val form = state.form ?: return@forEach
        if (!state.redraw(instance.accountPage(form.accountId)?.separateServer)) separateLinked(state)
    }
}

/** Connect: what was typed, tried by the core as a change on the account's page. */
internal fun MainActivity.connectSeparate(state: SeparateSignIn, username: String, password: String) {
    val form = state.form ?: return
    if (!state.sending()) return
    changeAccount(AccountPageIntent.ConnectSeparateServer(form.accountId, username, TypedPassword(password)))
}

/** Setup's step: Skip switches off exactly the uses the sign-in did not reach, then goes on. */
internal fun MainActivity.skipSetupSeparate() {
    val state = setupFlow.separate
    val form = state.form ?: return
    if (app == null) {
        if (!setupFlow.separateDone(::linkStepFor)) finishSetup()
        return
    }
    if (!state.sending(skip = true)) return
    changeAccount(AccountPageIntent.StopUsingSeparateServer(form.accountId))
}

/** The account page's action: the form for every use of [accountId] that needs this. */
internal fun MainActivity.openPageSeparate(accountId: String) {
    val section = app?.accountPage(accountId)?.separateServer ?: return
    UnreachedRules.form(accountId, section)?.let { accountSettings.separate.open(it) }
}
