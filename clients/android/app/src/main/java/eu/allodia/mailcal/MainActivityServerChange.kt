// A calendar or address book server found on another site (docs/account-autodetect.md rule 14):
// the question opened from the calendar, the contacts list or the account's page, and Allow, which
// connects the new server and so runs off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread

/** Opens the question for [accountId]; one no longer waiting redraws the lines instead. */
internal fun MainActivity.reviewServerChange(accountId: String) {
    val instance = app ?: return
    val change = instance.pendingServerChange(accountId)
    if (change == null) {
        refreshAccountsSnapshot()
        refreshConnectivity()
        return
    }
    val address = accountSettings.addressOf(accountId, accounts.firstOrNull { it.id == accountId }?.email)
    accountSettings.serverChange.open(ShownServerChange(accountId, address, change))
}

/** Allow: the pause lifts once the new server has connected; a failure changes nothing. */
internal fun MainActivity.allowServerChange() {
    val instance = app ?: return
    val prompt = accountSettings.serverChange
    val accountId = prompt.shown?.accountId ?: return
    if (!prompt.start()) return
    thread(name = "mailcal-server-change") {
        val answer = runCatching { instance.acceptServerChange(accountId) }
        mainHandler.post {
            answer.onSuccess {
                logUiInfo("accounts: a calendar or address book moved to its new server")
                refreshAccountsSnapshot()
                refreshConnectivity()
            }
            answer.onFailure {
                logUiWarn("accounts: a new calendar or address book server did not connect: ${coreErrorText(it)}")
            }
            prompt.finished(accountId, answer.isSuccess)
        }
    }
}
