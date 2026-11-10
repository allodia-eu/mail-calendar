// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). A use, a link and a removal
// are the core's to run (docs/waiting.md): dispatched, they answer at once and the page says they
// run. Every other change may touch the Keystore or dial a server, so it runs off the main thread
// and answers with what the page should say.
package eu.allodia.mailcal

import android.widget.Toast
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountChangeNoticeRaised
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.LinkSlot

/**
 * Reads the snapshot again. The last account gone returns the app to first-run setup, which is
 * decided here rather than from the mailbox: an account without mail is in none of its rows
 * (docs/accounts.md rule 11).
 */
internal fun MainActivity.refreshAccountsSnapshot() {
    val instance = app ?: return
    val snapshot = instance.accountsSnapshot()
    accountSettings.snapshot = snapshot
    val any = snapshot.accounts.isNotEmpty()
    if (!any && accountSettings.hadAccounts) {
        logUiInfo("accounts: the last account is gone; returning to first-run setup")
        showingSettings = false
        settingsCategory = null
        addingAccount = false
        accountSettings.page = null
        needsSetup = true
    }
    accountSettings.hadAccounts = any
    if (accountSettings.page?.let { accountSettings.entry(it) } == null) {
        accountSettings.page = null
    }
    refreshAccountPage()
    refreshServerChange()
    // A suggestion the core found after the link step was drawn reaches it.
    setupFlow.links?.let { setupFlow.refresh(linkStepFor(it.account)) }
}

/** Opens Settings on account [id]'s page: where a password is replaced and a use asked for. */
internal fun MainActivity.openAccountPage(id: String) {
    accountSettings.notice = null
    accountSettings.page = id
    refreshAccountPage()
    settingsCategory = SettingsCategory.ACCOUNTS
    showingSettings = true
}

/**
 * Reads the open account's page again, and says what a change came to, once: on its page, or for a
 * removal, wherever the person is.
 */
internal fun MainActivity.refreshAccountPage() {
    val instance = app ?: return
    val page = accountSettings.page?.let { instance.accountPage(it) }
    accountSettings.accountPage = page
    // The core withdraws what a change came to when the page's next change starts: a save that
    // worked must not stand under the failure before it.
    if (page != null && page.notice == null && accountSettings.notice === accountSettings.changeNotice) {
        accountSettings.notice = null
    }
    page?.notice?.let(::sayAccountNotice)
    instance.accountsNotice()?.let(::sayAccountNotice)
}

private fun MainActivity.sayAccountNotice(raised: AccountChangeNoticeRaised) {
    if (raised.id <= accountSettings.coreNotice) return
    accountSettings.coreNotice = raised.id
    logUiWarn("accounts: a change on an account's page did not take")
    val said = changeNotice(this, raised.notice)
    say(said)
    accountSettings.changeNotice = said
}

/** Asks the core for a change on an account's page; it answers at once, and the page says it runs. */
internal fun MainActivity.changeAccount(intent: AccountPageIntent) {
    app?.dispatch(Intent.AccountPage(intent))
}

/** Makes the links setup's link step chose, one after another. */
internal fun MainActivity.linkFromSetup(accountId: String, links: List<Pair<LinkSlot, String>>) {
    val instance = app ?: return
    thread(name = "mailcal-setup-links") {
        // Stored through the Keystore, which may block.
        links.forEach { (slot, target) ->
            runCatching { instance.setAccountLink(accountId, slot, target) }
                .onFailure { logUiWarn("setup: linking an account failed: ${coreErrorText(it)}") }
        }
        mainHandler.post { refreshAccountsSnapshot() }
    }
}

/**
 * Says [notice] where the person is: on the account's page while Settings is open, and in passing
 * otherwise, since a change can end after Settings closed and a notice kept for it would greet
 * them there later about something long over.
 */
internal fun MainActivity.say(notice: AccountNotice) {
    if (showingSettings) {
        accountSettings.notice = notice
    } else {
        val text = listOf(notice.title, notice.message).filter { it.isNotEmpty() }.joinToString(": ")
        Toast.makeText(this, text, Toast.LENGTH_LONG).show()
    }
}
