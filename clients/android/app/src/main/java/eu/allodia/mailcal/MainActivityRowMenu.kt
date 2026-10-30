// What the activity does with a row menu's pick (docs/row-actions.md). Which item was picked and
// which message it names are the core's answer, read by RowMenuPresentation.kt; this file runs it.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MenuMessage
import uniffi.mailcal_bindings.SnapshotRow

internal fun MainActivity.performRowPick(instance: MailcalApp, pick: RowPick) {
    when (pick) {
        is RowPick.Open -> openMenuTarget(instance, pick.message)
        is RowPick.Answer -> openComposer(instance, pick.origin)
        is RowPick.Write -> instance.dispatch(pick.intent)
        is RowPick.Rows -> instance.dispatch(Intent.ActOnSelection(pick.rows, pick.action))
        // The row asks for the folder itself, and moves through its own filing.
        is RowPick.Move -> Unit
    }
}

/**
 * Opens the message an item names, as tapping its row would: a conversation's message on the
 * conversation's screen, a draft into its composer.
 */
private fun MainActivity.openMenuTarget(instance: MailcalApp, target: MenuMessage) {
    val zone = timeZone?.active
    val use24Hour = uses24HourClock()
    for (row in rows) {
        when (row) {
            is SnapshotRow.Flat -> if (row.row.account == target.account && row.row.key == target.key) {
                val flat = row.row
                val opened = OpenedMessage(
                    account = flat.account,
                    key = flat.key,
                    subject = flat.subject,
                    from = flat.from,
                    avatar = flat.avatar,
                    date = localDateTime(flat.date, zone, use24Hour),
                )
                openOrResume(instance, opened, ListedOpening.resumes(target))
                return
            }
            is SnapshotRow.Thread -> {
                val thread = row.row
                val message = thread.messages.firstOrNull {
                    it.account == target.account && it.key == target.key
                } ?: continue
                openOrResume(
                    instance,
                    threadHead(thread, message),
                    ListedOpening.resumes(target),
                    thread.messages,
                )
                return
            }
        }
    }
}
