// The shared mailboxes on an account's page (docs/shared-mailboxes.md rules 3, 7 and 8): what each
// account's server answered when its page opened, the switch or address the core is still
// answering, and how each mailbox and the line beneath them read. Held by the activity beside the
// rest of Settings → Accounts, because the server's answer outlives every redraw of the page;
// catalog-free in its decisions, so the JVM suite reaches every rule.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.SharedMailboxEntry
import uniffi.mailcal_bindings.SharedMailboxSupport
import uniffi.mailcal_bindings.SharedMailboxesView

/** Which line, if any, sits beneath an account's shared mailboxes. */
internal enum class SharedNote {
    NONE,

    /** The server lists them, and lists none. */
    NO_MAILBOXES,

    /** The server lists none, but opens one at a typed address. */
    BY_ADDRESS,

    /** The account has not connected yet, so nothing has been asked. */
    NOT_CONNECTED,

    /** The server was asked and did not answer. */
    UNREACHABLE,
}

/**
 * One mailbox's row: [title] is its address where it has one, [label] the server's own name for it
 * when that says something else, [withdrawn] whether the server no longer lists it.
 */
internal data class SharedRow(
    val title: String,
    val label: String?,
    val withdrawn: Boolean,
)

/** A mailbox switch the core is still answering, drawn at its new position until it does. */
internal data class PendingShared(val accountId: String, val handle: String, val on: Boolean)

internal object SharedMailboxRules {
    /**
     * Whether the page draws the section: once the server has answered, and not for an account
     * whose sign-in opens none and holds none, which would be a section that says nothing.
     */
    fun shows(view: SharedMailboxesView?): Boolean =
        view != null && !(view.support == SharedMailboxSupport.UNSUPPORTED && view.mailboxes.isEmpty())

    /** The row for [mailbox], named by its address where it has one. */
    fun row(mailbox: SharedMailboxEntry): SharedRow {
        val title = mailbox.address ?: mailbox.name
        return SharedRow(
            title = title,
            label = mailbox.name.takeIf { mailbox.listed && it != title },
            withdrawn = !mailbox.listed,
        )
    }

    /** The line beneath the switches, when the answer needs one. */
    fun note(view: SharedMailboxesView): SharedNote = when (view.support) {
        SharedMailboxSupport.LISTED ->
            if (view.mailboxes.isEmpty()) SharedNote.NO_MAILBOXES else SharedNote.NONE
        SharedMailboxSupport.UNSUPPORTED -> SharedNote.NONE
        SharedMailboxSupport.BY_ADDRESS -> SharedNote.BY_ADDRESS
        SharedMailboxSupport.NOT_CONNECTED -> SharedNote.NOT_CONNECTED
        SharedMailboxSupport.UNREACHABLE -> SharedNote.UNREACHABLE
    }

    /** Whether the page offers a field for a mailbox's address. */
    fun takesAddress(view: SharedMailboxesView): Boolean =
        view.support == SharedMailboxSupport.BY_ADDRESS

    /**
     * Whether asking again may answer something new: the account was not connected yet, or could
     * not open a shared mailbox, which a sign-in completed changes; or what it is used for changed
     * since [asked], and switching mail or shared mailboxes off removes every one chosen. Anything
     * else would be a request to the server on every signal.
     */
    fun mayHaveChanged(
        view: SharedMailboxesView?,
        asked: List<AccountCapability>?,
        uses: List<AccountCapability>,
    ): Boolean =
        view == null ||
            asked != uses ||
            view.support == SharedMailboxSupport.NOT_CONNECTED ||
            view.support == SharedMailboxSupport.UNSUPPORTED
}

internal class SharedMailboxesState {
    /** What each account's server last answered, by account id. */
    var answers by mutableStateOf<Map<String, SharedMailboxesView>>(emptyMap())
        private set

    private val asking = mutableSetOf<String>()

    /** What each account was used for when its server was last asked. */
    private val asked = mutableMapOf<String, List<AccountCapability>>()

    /** A switch the core is still answering. */
    var pending by mutableStateOf<PendingShared?>(null)

    /** The account whose typed address the server is being asked about. */
    var adding by mutableStateOf<String?>(null)

    /** Counts mailboxes added by address, so the field empties after each. */
    var added by mutableIntStateOf(0)
        private set

    /** Whether the core is answering a switch or an address, which holds every other still. */
    val busy: Boolean get() = pending != null || adding != null

    /**
     * Whether to ask [accountId]'s server now, while it is used for [uses]: false while a question
     * is already out.
     */
    fun startAsking(accountId: String, uses: List<AccountCapability>): Boolean {
        if (!asking.add(accountId)) return false
        asked[accountId] = uses
        return true
    }

    /**
     * Whether to ask [accountId]'s server again on a signal, now that it is used for [uses]: only
     * while its answer may change.
     */
    fun asksAgain(accountId: String, uses: List<AccountCapability>): Boolean =
        accountId !in asking &&
            SharedMailboxRules.mayHaveChanged(answers[accountId], asked[accountId], uses)

    /** The server answered for [accountId]; null is an account that is no longer set up. */
    fun answered(accountId: String, view: SharedMailboxesView?) {
        asking.remove(accountId)
        // An answer equal to the last one leaves the page alone.
        if (answers[accountId] == view) return
        answers = if (view == null) answers - accountId else answers + (accountId to view)
    }

    /** A typed address was added. */
    fun addedOne() {
        added += 1
    }
}
