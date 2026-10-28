// The shared mailboxes on an account's page, asked of the core (docs/shared-mailboxes.md): which the
// account's sign-in opens, asked as its page opens and again while the answer may still change, and
// one switched on, off, or added by its address. Asking is a request to the server, and switching
// stores the account and may delete what the device holds, so each runs off the main thread.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.SharedMailboxEntry

/** Asks which mailboxes account [accountId]'s sign-in opens besides its own. */
internal fun MainActivity.askSharedMailboxes(accountId: String) {
    val instance = app ?: return
    val shared = accountSettings.shared
    if (!shared.startAsking(accountId, inUse(accountId))) return
    thread(name = "mailcal-shared-mailboxes") {
        val read = shared.read()
        // A refusal is an account that is not set up: it has no section.
        val view = runCatching { instance.sharedMailboxes(accountId) }.getOrNull()
        mainHandler.post { shared.answered(accountId, view, read) }
    }
}

/**
 * Asks the open account page's server again when its answer may have changed: the account has
 * connected since, or a use was switched on or off. The signals that announce either call this,
 * after the accounts snapshot is read again.
 */
internal fun MainActivity.askSharedMailboxesAgain() {
    if (!showingSettings) return
    val accountId = accountSettings.page ?: return
    if (accountSettings.shared.asksAgain(accountId, inUse(accountId))) askSharedMailboxes(accountId)
}

private fun MainActivity.inUse(accountId: String) =
    accountSettings.entry(accountId)?.let(AccountSettingsRules::inUse).orEmpty()

/** Uses account [accountId] for [mailbox], or stops, and reads the answer again after. */
internal fun MainActivity.setSharedMailbox(accountId: String, mailbox: SharedMailboxEntry, on: Boolean) {
    val instance = app ?: return
    val shared = accountSettings.shared
    if (shared.busy) return
    shared.pending = PendingShared(accountId, mailbox.handle, on)
    thread(name = "mailcal-shared-mailbox-set") {
        val answer = runCatching { instance.setSharedMailbox(accountId, mailbox, on) }
        val read = shared.read()
        val view = runCatching { instance.sharedMailboxes(accountId) }.getOrNull()
        mainHandler.post {
            shared.pending = null
            shared.answered(accountId, view, read)
            answer.onFailure {
                // The event alone: the core's words can quote the typed address (docs/logging.md).
                logUiWarn("accounts: switching a shared mailbox failed")
                accountSettings.notice = changeFailed(coreErrorSaid(it, this))
            }
            refreshAccountsSnapshot()
        }
    }
}

/**
 * Asks the server for the mailbox at [address] and, when account [accountId]'s sign-in opens it,
 * uses the account for it. A refusal leaves the address where it was typed, to correct.
 */
internal fun MainActivity.addSharedMailbox(accountId: String, address: String) {
    val instance = app ?: return
    val shared = accountSettings.shared
    if (shared.busy) return
    shared.adding = accountId
    thread(name = "mailcal-shared-mailbox-add") {
        val answer = runCatching {
            instance.resolveSharedMailbox(accountId, address)?.also { found ->
                instance.setSharedMailbox(accountId, found, true)
            }
        }
        val read = shared.read()
        val view = runCatching { instance.sharedMailboxes(accountId) }.getOrNull()
        mainHandler.post {
            shared.adding = null
            shared.answered(accountId, view, read)
            answer.onSuccess { found ->
                if (found != null) {
                    shared.addedOne()
                } else {
                    accountSettings.notice = AccountNotice(
                        NoticeKind.WARNING,
                        "",
                        L10n.settings_account_shared_not_found(this),
                    )
                }
            }
            answer.onFailure {
                // The event alone: a Graph refusal quotes the address typed (docs/logging.md).
                logUiWarn("accounts: adding a shared mailbox failed")
                accountSettings.notice = changeFailed(coreErrorSaid(it, this))
            }
            refreshAccountsSnapshot()
        }
    }
}

