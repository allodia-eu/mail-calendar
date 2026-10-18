// A calendar or address book an account's sign-in does not reach (docs/accounts.md rule 18): the
// question a sign-in that will not reach them asks first, and the form that gives them a sign-in
// of their own, as setup's step once a mailbox has connected and on the account's page. Compose-free and catalog-free,
// so the JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.UnreachedUses

/** Which of calendar and contacts a title names. */
internal enum class UnreachedSet { BOTH, CALENDAR, CONTACTS }

/** The three answers to the question a switch asks first. */
internal enum class UnreachedChoice { KEEP, STOP, CANCEL }

/**
 * The form that gives [uses] of account [accountId] a sign-in of their own at [host], prefilled
 * with [username].
 */
internal data class SeparateServerForm(
    val accountId: String,
    val uses: List<AccountCapability>,
    val set: UnreachedSet,
    val host: String,
    val username: String,
)

internal object UnreachedRules {
    private val DAV = listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)

    /** The set [uses] names, or null when it names neither calendar nor contacts. */
    fun setOf(uses: List<AccountCapability>): UnreachedSet? {
        val calendar = AccountCapability.CALENDAR in uses
        val contacts = AccountCapability.CONTACTS in uses
        return when {
            calendar && contacts -> UnreachedSet.BOTH
            calendar -> UnreachedSet.CALENDAR
            contacts -> UnreachedSet.CONTACTS
            else -> null
        }
    }

    /** Whether a sign-in that will not reach [unreached] asks before the browser opens. */
    fun asks(unreached: List<AccountCapability>): Boolean = setOf(unreached) != null

    /** What [choice] tells the core, or null for Cancel, which abandons the sign-in. */
    fun answer(choice: UnreachedChoice): UnreachedUses? = when (choice) {
        UnreachedChoice.KEEP -> UnreachedUses.KEEP_ON_PASSWORD
        UnreachedChoice.STOP -> UnreachedUses.STOP_USING
        UnreachedChoice.CANCEL -> null
    }

    /**
     * The form that gives a mailbox's calendar or contacts a sign-in of their own, when the core
     * names a server they need one at: what setup shows once the account has connected, before
     * its link step, and what the account's page offers. Null when nothing needs one.
     */
    fun form(entry: AccountEntry?): SeparateServerForm? {
        val server = entry?.separateServer ?: return null
        val uses = server.uses.filter { it in DAV }
        val set = setOf(uses) ?: return null
        return SeparateServerForm(entry.id, uses, set, server.host, server.username.ifBlank { entry.address })
    }

    /** Whether [capability]'s row on the account's page offers the form. */
    fun offersSeparate(entry: AccountEntry, capability: AccountCapability): Boolean =
        form(entry)?.uses?.contains(capability) == true

    /** What Skip switches, as (use, on): each use the sign-in did not reach, off. */
    fun skipChanges(unreached: List<AccountCapability>): List<Pair<AccountCapability, Boolean>> =
        unreached.filter { it in DAV }.distinct().map { it to false }
}
