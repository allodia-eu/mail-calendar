// A calendar or address book server found on another site, waiting for the person to allow it
// (docs/account-autodetect.md rule 14): which words the dialog says, and the question's state,
// held by the activity because the calendar, the contacts list and the account's page all open it.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontWeight
import uniffi.mailcal_bindings.ServerChange
import uniffi.mailcal_bindings.ServerChangeAnswer

/** The dialog's words, in the order it draws them. */
internal data class ServerChangeCopy(
    val title: String,
    val body: String,
    val before: String,
    val now: String,
    val advice: String,
    val unsure: String,
)

internal object ServerChangeRules {
    /** Which of calendar and contacts moved. The core names at least one. */
    fun set(change: ServerChange): UnreachedSet = when {
        change.calendar && change.contacts -> UnreachedSet.BOTH
        change.contacts -> UnreachedSet.CONTACTS
        else -> UnreachedSet.CALENDAR
    }

    /** What the dialog says about [change] on the account read as [address]. */
    fun copy(ctx: Context, change: ServerChange, address: String): ServerChangeCopy {
        val set = set(change)
        return ServerChangeCopy(
            title = when (set) {
                UnreachedSet.BOTH -> L10n.server_changed_title_both(ctx)
                UnreachedSet.CALENDAR -> L10n.server_changed_title_calendar(ctx)
                UnreachedSet.CONTACTS -> L10n.server_changed_title_contacts(ctx)
            },
            body = when (set) {
                UnreachedSet.BOTH -> L10n.server_changed_body_both(ctx, address)
                UnreachedSet.CALENDAR -> L10n.server_changed_body_calendar(ctx, address)
                UnreachedSet.CONTACTS -> L10n.server_changed_body_contacts(ctx, address)
            },
            before = L10n.server_changed_before(ctx, change.previousSite),
            now = L10n.server_changed_now(ctx, change.proposedSite),
            advice = L10n.server_changed_advice(ctx),
            unsure = when (set) {
                UnreachedSet.BOTH -> L10n.server_changed_unsure_both(ctx)
                UnreachedSet.CALENDAR -> L10n.server_changed_unsure_calendar(ctx)
                UnreachedSet.CONTACTS -> L10n.server_changed_unsure_contacts(ctx)
            },
        )
    }

    /** [line] with [site] in bold where it appears, so the two sites stand out to compare. */
    fun emphasised(line: String, site: String): AnnotatedString {
        val start = line.lastIndexOf(site)
        if (site.isEmpty() || start < 0) return AnnotatedString(line)
        val bold = AnnotatedString.Range(SpanStyle(fontWeight = FontWeight.Bold), start, start + site.length)
        return AnnotatedString(line, spanStyles = listOf(bold))
    }
}

/** The change being asked about, for account [accountId] read as [address]. */
internal data class ShownServerChange(
    val accountId: String,
    val address: String,
    val change: ServerChange,
)

internal class ServerChangePrompt {
    /** The question on screen, or null. */
    var shown by mutableStateOf<ShownServerChange?>(null)
        private set

    /** Whether Allow is connecting the new server; the dialog holds still meanwhile. */
    var allowing by mutableStateOf(false)
        private set

    /** Whether the last Allow failed, which the dialog says. */
    var failed by mutableStateOf(false)
        private set

    fun open(question: ShownServerChange) {
        shown = question
        allowing = false
        failed = false
    }

    /** Whether Allow may start now: a question is open and no Allow runs. */
    fun start(): Boolean {
        if (shown == null || allowing) return false
        allowing = true
        failed = false
        return true
    }

    /**
     * Allow answered for [accountId]; an answer for a question no longer open is dropped. A question
     * that was replaced stays open, asking the new one.
     */
    fun finished(accountId: String, answer: Result<ServerChangeAnswer>) {
        val question = shown ?: return
        if (question.accountId != accountId) return
        allowing = false
        when (val answered = answer.getOrNull()) {
            null -> failed = true
            is ServerChangeAnswer.Settled -> shown = null
            is ServerChangeAnswer.Replaced -> shown = question.copy(change = answered.change)
        }
    }

    /** Not now, or the question closing: nothing is asked of the core. */
    fun close() {
        if (allowing) return
        shown = null
        failed = false
    }
}
