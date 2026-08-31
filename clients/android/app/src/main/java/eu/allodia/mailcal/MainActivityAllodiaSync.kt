// Keeping this device's mail-account list in step with the person's other devices, driven from the
// activity. The core does the deciding and the writing; what is here is when to ask it and what to
// do with the part it cannot answer alone.
//
// The pass BLOCKS on the network, so it runs off the main thread, like every other core call that
// reaches a server.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AllodiaGrantHealth
import uniffi.mailcal_bindings.AllodiaSyncReport
import uniffi.mailcal_bindings.MailcalApp

/**
 * What the accounts screen draws about the person's other devices.
 *
 * [report] is null until a pass has run. That is not the same as a pass that found nothing, which
 * is an empty report, the first draws no section at all, the second draws none either but has
 * earned it.
 */
internal data class AllodiaSyncState(
    val checking: Boolean = false,
    val report: AllodiaSyncReport? = null,
    val failure: String? = null,
    /**
     * What the core knows about the sign-in itself, which is what a failure is DRAWN from.
     *
     * [failure] says a pass did not finish; this says whether that is the person's business and
     * what they can do about it. Putting the failure's own text on screen is how a generated OAuth
     * field name became product copy.
     */
    val health: AllodiaGrantHealth = AllodiaGrantHealth.OK,
)

/**
 * Hand the core somewhere to remember what it has synced.
 *
 * Called on the connect thread, before anything asks for a pass. A dev-account launch gets its own
 * preferences file, so a harness mailbox and a real one can never end up in the same bookkeeping.
 */
internal fun MainActivity.installAllodiaSyncStore(instance: MailcalApp, dataSubdir: String?) {
    try {
        instance.useAllodiaSyncStateStore(
            SharedPrefsSyncStateStore(this, syncStateFile(dataSubdir)),
        )
    } catch (e: Exception) {
        // The blob could not be read. Syncing is off for this launch rather than starting from
        // nothing, which would re-adopt every record and re-offer every account.
        logUiWarn("allodia: the sync state could not be read (${e.message}); not syncing this launch")
    }
}

/**
 * Run one pass, if there is any point in running one.
 *
 * Nobody signed in, no registration in this build, or a dev-account launch: there is nothing to
 * sync against, and asking would only produce an error to draw. A pass already running is left to
 * finish, two at once would race each other's writes.
 */
internal fun MainActivity.syncAllodiaAccounts() {
    val activity = this
    val instance = activity.app ?: return
    if (activity.allodiaAccount == null || activity.allodiaSync.checking) return
    if (!activity.allodiaSyncAllowed) return
    activity.allodiaSync = activity.allodiaSync.copy(checking = true, failure = null)
    thread(name = "mailcal-allodia-sync") {
        try {
            val report = instance.syncAllodiaAccounts()
            logUiInfo(
                "allodia: sync done, ${report.sent} sent, ${report.offers.size} offered, " +
                    "${report.changedElsewhere.size} changed elsewhere, " +
                    "${report.removedElsewhere.size} removed elsewhere",
            )
            val health = instance.allodiaGrantHealth()
            activity.mainHandler.post {
                activity.allodiaSync = AllodiaSyncState(report = report, health = health)
            }
        } catch (e: Exception) {
            logUiWarn("allodia: the sync pass did not finish (${e.message})")
            // The core's typed answer, not this exception's text: it is what the screen draws, and
            // it stays as it was when nothing was learned about the sign-in.
            val health = instance.allodiaGrantHealth()
            activity.mainHandler.post {
                activity.allodiaSync = activity.allodiaSync.copy(
                    checking = false,
                    failure = e.message.orEmpty(),
                    health = health,
                )
            }
        }
    }
}

/**
 * Move one account to a sync position.
 *
 * The core does everything the position takes, including reaching the service, so this BLOCKS
 * and runs off the main thread. The rows asking about an account changed or removed elsewhere go
 * as soon as it is answered: a question still on screen afterwards reads as the answer not having
 * worked.
 *
 * The position on screen is re-read from the core rather than assumed, so a change the service
 * refused leaves the control where it was instead of lying about what happened.
 */
internal fun MainActivity.setAllodiaAccountSyncMode(
    accountId: String,
    mode: AllodiaAccountSyncMode,
) {
    val activity = this
    val instance = activity.app ?: return
    thread(name = "mailcal-allodia-mode") {
        val failure = try {
            instance.setAllodiaAccountSyncMode(accountId, mode)
            null
        } catch (e: Exception) {
            logUiWarn("allodia: the account's sync position could not be set (${e.message})")
            e.message.orEmpty()
        }
        activity.mainHandler.post {
            activity.readAccountsSynced()
            activity.allodiaSync = activity.allodiaSync.copy(failure = failure)
            if (failure == null) {
                activity.allodiaSync.report?.let { report ->
                    activity.allodiaSync = activity.allodiaSync.copy(
                        report = AllodiaSyncReport(
                            offers = report.offers,
                            changedElsewhere =
                                report.changedElsewhere.filter { it.accountId != accountId },
                            removedElsewhere =
                                report.removedElsewhere.filter { it.accountId != accountId },
                            sent = report.sent,
                        ),
                    )
                }
            }
        }
    }
}

/** Where each account stands. A local read per account; it never asks the service. */
internal fun MainActivity.readAccountsSynced() {
    val instance = app ?: return
    accountsSyncMode = syncSettings?.accounts.orEmpty().associate { account ->
        account.accountId to instance.allodiaAccountSyncMode(account.accountId)
    }
}

