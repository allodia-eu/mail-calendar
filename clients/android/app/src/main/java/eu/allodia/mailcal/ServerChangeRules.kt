// A calendar or address book server found on another site, waiting for the person to allow it
// (docs/account-autodetect.md rule 14): which words the dialog says, and which account's question
// is open, held by the activity because the calendar, the contacts list and the account's page all
// open it.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontWeight
import uniffi.mailcal_bindings.AccountServerChange
import uniffi.mailcal_bindings.ServerChange

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

/** The question on screen, for account [accountId], as the core drew it last. */
internal data class ShownServerChange(
    val accountId: String,
    val question: AccountServerChange,
)

/**
 * Which account's question is open. What it says, whether Allow runs and whether the last one
 * failed are the core's ([AccountServerChange]), read again on every Settings and Notices signal.
 */
internal class ServerChangePrompt {
    /** The question on screen, or null. */
    var shown by mutableStateOf<ShownServerChange?>(null)
        private set

    fun open(question: ShownServerChange) {
        shown = question
    }

    /** What the core says of the open question now; null once the account waits on none, which closes it. */
    fun redraw(question: AccountServerChange?) {
        val open = shown ?: return
        shown = question?.let { open.copy(question = it) }
    }

    /** Not now, or the question closing: nothing is asked of the core. Not while Allow runs, which closing would not stop. */
    fun close() {
        if (shown?.question?.allow?.running == true) return
        shown = null
    }
}
