// A calendar or address book server found on another site (docs/account-autodetect.md rule 14):
// the question opened from the calendar, the contacts list or the account's page, drawn from the
// core's record. Allow is a change on the account's page, so the core runs it off this thread.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountPageIntent

/** Opens the question for [accountId]; one no longer waiting redraws the lines instead. */
internal fun MainActivity.reviewServerChange(accountId: String) {
    val instance = app ?: return
    val question = instance.accountServerChange(accountId)
    if (question == null) {
        refreshAccountsSnapshot()
        pullNotices()
        return
    }
    val address = accountSettings.addressOf(accountId, accounts.firstOrNull { it.id == accountId }?.email)
    accountSettings.serverChange.open(ShownServerChange(accountId, address, question))
}

/** Reads the open question again: an allow starting or ending, or a lookup replacing it. */
internal fun MainActivity.refreshServerChange() {
    val instance = app ?: return
    val prompt = accountSettings.serverChange
    val open = prompt.shown ?: return
    prompt.redraw(instance.accountServerChange(open.accountId))
}

/** Allow: the change the question shows, which the core moves the account onto only once it connects. */
internal fun MainActivity.allowServerChange(shown: ShownServerChange) {
    changeAccount(AccountPageIntent.AllowServerChange(shown.accountId, shown.question.change))
}
