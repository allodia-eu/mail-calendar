// What Settings → Accounts says about an account, decided from its snapshot entry alone
// (docs/accounts.md rules 11 to 13). Compose-free and catalog-free on purpose, so the JVM suite
// reaches every rule: the words are the page's, the decisions are here.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.CapabilityChange
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.UseProblem
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/** Which line, if any, sits under a use's switch. */
internal enum class UseNote {
    NONE,

    /** The last of mail, calendar and contacts: the account is removed instead. */
    LAST,

    /** Colleagues, while contacts are off. */
    NEEDS_CONTACTS,

    /** Chosen, and the provider has not granted it. */
    WITHHELD,

    /** The server offers none, so nothing can switch it on. */
    NOT_OFFERED,

    /** On, and its server could not be reached; the next refresh tries again. */
    UNREACHABLE_CALENDAR,
    UNREACHABLE_CONTACTS,

    /** On, and its server refused the account's sign-in. */
    REFUSED_CALENDAR,
    REFUSED_CONTACTS,

    /** On, and its server needs a sign-in of its own. */
    OWN_SIGN_IN_CALENDAR,
    OWN_SIGN_IN_CONTACTS,
}

/**
 * What a use that did not connect says beneath its switch, and on the screen that shows it. Only a
 * calendar or an address book fails on its own; mail decides whether the account connected at all.
 */
internal fun problemNote(capability: AccountCapability, problem: UseProblem): UseNote {
    val calendar = capability == AccountCapability.CALENDAR
    return when (problem) {
        UseProblem.UNREACHABLE ->
            if (calendar) UseNote.UNREACHABLE_CALENDAR else UseNote.UNREACHABLE_CONTACTS
        UseProblem.SIGN_IN_REFUSED ->
            if (calendar) UseNote.REFUSED_CALENDAR else UseNote.REFUSED_CONTACTS
        UseProblem.NEEDS_OWN_SIGN_IN ->
            if (calendar) UseNote.OWN_SIGN_IN_CALENDAR else UseNote.OWN_SIGN_IN_CONTACTS
    }
}

/**
 * How one use's switch is drawn. [active] is on; a use waiting on a permission is drawn off,
 * because it is not working and switching it on is what asks the provider again. [asks] says
 * switching it on asks the provider rather than the core, and [enabled] whether it can be flipped.
 */
internal data class UseSwitch(
    val active: Boolean,
    val asks: Boolean,
    val enabled: Boolean,
    val note: UseNote,
)

/**
 * One link slot's picker: "none" first, then [options]. [selected] is the index into [options] of
 * the account linked now, null for "none"; [suggested] the index to suggest while nothing is
 * linked, one whose calendar server schedules as the mail account's address.
 */
internal data class LinkPicker(
    val slot: LinkSlot,
    val options: List<LinkedAccount>,
    val selected: Int?,
    val suggested: Int?,
)

/** What the page does once switching a use has answered. */
internal enum class UseChangeOutcome {
    /** Done; the page redraws from the next snapshot. */
    APPLIED,

    /** The provider has not granted it: sign the account in again, asking for it. */
    ASK_PROVIDER,

    /** The account has no server for it. */
    NEEDS_ENDPOINT,
}

internal object AccountSettingsRules {
    private val PRIMARY = listOf(
        AccountCapability.MAIL,
        AccountCapability.CALENDAR,
        AccountCapability.CONTACTS,
    )

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
     * The switch for [capability]. The last of mail, calendar and contacts cannot be switched off,
     * and colleagues cannot be switched on without contacts: the core refuses both, so the switch
     * says why rather than letting the person find out.
     */
    fun switch(entry: AccountEntry, capability: AccountCapability): UseSwitch {
        fun used(wanted: AccountCapability) =
            entry.uses.any { it.capability == wanted && works(it.state) }
        val state = entry.uses.firstOrNull { it.capability == capability }?.state
            ?: CapabilityState.OFF
        // A use waiting on a permission is drawn off, so it is never the one held on: switching it
        // on asks the provider, which the core does not refuse.
        val last = state == CapabilityState.ON &&
            capability in PRIMARY &&
            PRIMARY.count(::used) == 1
        val withoutContacts = capability == AccountCapability.COLLEAGUES &&
            state == CapabilityState.OFF &&
            !used(AccountCapability.CONTACTS)
        val notOffered = state == CapabilityState.NOT_OFFERED
        val problem = entry.uses.firstOrNull { it.capability == capability }?.problem
        val note = when {
            notOffered -> UseNote.NOT_OFFERED
            problem != null -> problemNote(capability, problem)
            last -> UseNote.LAST
            withoutContacts -> UseNote.NEEDS_CONTACTS
            state == CapabilityState.NEEDS_PERMISSION -> UseNote.WITHHELD
            else -> UseNote.NONE
        }
        return UseSwitch(
            active = state == CapabilityState.ON,
            asks = state == CapabilityState.NEEDS_PERMISSION,
            enabled = !last && !withoutContacts && !notOffered,
            note = note,
        )
    }

    /**
     * The pickers an account's page offers: one per slot it can hold, which is a slot with
     * candidates or one already linked. A link the candidates no longer offer is kept as the
     * first option, so the picker shows what is stored rather than "none". Matched by id, so a
     * stored link whose address has since changed is not listed twice.
     */
    fun linkPickers(entry: AccountEntry): List<LinkPicker> {
        val slots = listOf(
            Triple(LinkSlot.CALENDAR, entry.linkCandidates.calendar, entry.links.calendar),
            Triple(LinkSlot.CONTACTS, entry.linkCandidates.contacts, entry.links.contacts),
            Triple(LinkSlot.MAIL, entry.linkCandidates.mail, entry.links.mail),
        )
        return slots.mapNotNull { (slot, candidates, current) ->
            if (candidates.isEmpty() && current == null) return@mapNotNull null
            val options = candidates.toMutableList()
            if (current != null && options.none { it.id == current.id }) {
                options.add(0, current)
            }
            val selected = current?.let { linked -> options.indexOfFirst { it.id == linked.id } }
            val suggested = if (selected == null) {
                options.indexOfFirst { it.id in entry.linkCandidates.suggested }
                    .takeIf { it >= 0 }
            } else {
                null
            }
            LinkPicker(slot, options, selected, suggested)
        }
    }

    /**
     * The accounts that lose their link when this one is removed, each as [name] names it, joined;
     * null when none does.
     */
    fun unlinked(entry: AccountEntry, name: (LinkedAccount) -> String): String? =
        entry.linkedFrom.takeIf { it.isNotEmpty() }?.joinToString(", ", transform = name)

    /** What switching a use on or off answered. */
    fun outcome(change: CapabilityChange): UseChangeOutcome = when (change) {
        CapabilityChange.NEEDS_CONSENT -> UseChangeOutcome.ASK_PROVIDER
        CapabilityChange.NEEDS_ENDPOINT -> UseChangeOutcome.NEEDS_ENDPOINT
        CapabilityChange.APPLIED -> UseChangeOutcome.APPLIED
    }
}
