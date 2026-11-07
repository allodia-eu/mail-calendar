// What Settings → Accounts' list says about an account, decided from its snapshot entry alone.
// Compose-free and catalog-free on purpose, so the JVM suite reaches every rule. An account's page
// is the core's (`account_page`): its switches, pickers and removal carry no rule here.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

internal object AccountSettingsRules {
    /** Whether a use in [state] is one the account is used for. One the server lacks is not. */
    private fun works(state: CapabilityState): Boolean =
        state == CapabilityState.ON || state == CapabilityState.NEEDS_PERMISSION

    /** The uses the account is used for, working or waiting on a permission, in snapshot order. */
    fun inUse(entry: AccountEntry): List<AccountCapability> =
        entry.uses.filter { works(it.state) }.map { it.capability }

    /** The links the account holds, as the list row names them. */
    fun linked(entry: AccountEntry): List<Pair<AccountCapability, LinkedAccount>> = listOfNotNull(
        entry.links.calendar?.let { AccountCapability.CALENDAR to it },
        entry.links.contacts?.let { AccountCapability.CONTACTS to it },
        entry.links.mail?.let { AccountCapability.MAIL to it },
    )

    /** Whether a use the account is used for waits on the provider's permission. */
    fun needsPermission(entry: AccountEntry): Boolean =
        entry.uses.any { it.state == CapabilityState.NEEDS_PERMISSION }

    /** The uses waiting on the provider's permission, for the notice a sign-in ends with. */
    fun withheld(entry: AccountEntry): List<AccountCapability> =
        entry.uses.filter { it.state == CapabilityState.NEEDS_PERMISSION }.map { it.capability }

    /** Whether the account signs in at its provider's own page, which "Sign in again" opens. */
    fun signsInAtProvider(kind: CoreAccountKind): Boolean =
        kind == CoreAccountKind.MICROSOFT || kind == CoreAccountKind.GOOGLE

    /**
     * The accounts that lose their link when this one is removed, each as [name] names it, joined;
     * null when none does.
     */
    fun unlinked(entry: AccountEntry, name: (LinkedAccount) -> String): String? =
        entry.linkedFrom.takeIf { it.isNotEmpty() }?.joinToString(", ", transform = name)
}
