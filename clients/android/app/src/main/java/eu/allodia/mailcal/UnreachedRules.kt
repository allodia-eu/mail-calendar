// A calendar or address book an account's sign-in does not reach (docs/accounts.md rule 18): the
// question a sign-in that will not reach them asks first, and the form that gives them a sign-in
// of their own, as setup's step once a mailbox has connected and on the account's page. Compose-free and catalog-free,
// so the JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.UnreachedUses

/** Which of calendar and contacts a title names. */
internal enum class UnreachedSet { BOTH, CALENDAR, CONTACTS }

/** The three answers to the question a switch asks first. */
internal enum class UnreachedChoice { KEEP, STOP, CANCEL }

/** The form that gives account [accountId]'s calendar or contacts a sign-in of their own. */
internal data class SeparateServerForm(
    val accountId: String,
    /** As the core last drew it. */
    val section: AccountSeparateServer,
    val set: UnreachedSet,
) {
    val uses: List<AccountCapability> get() = section.uses
}

internal object UnreachedRules {
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
     * The form for [accountId]'s [section], the core's for a mailbox whose calendar or contacts
     * need a sign-in of their own: what setup shows once the account has connected, before its link
     * step, and what the account's page opens. Null when nothing needs one.
     */
    fun form(accountId: String, section: AccountSeparateServer?): SeparateServerForm? {
        section ?: return null
        val set = setOf(section.uses) ?: return null
        return SeparateServerForm(accountId, section, set)
    }
}
