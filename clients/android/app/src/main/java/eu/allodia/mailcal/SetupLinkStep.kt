// The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
// another account's, and a calendar without mail can name the account that sends its invitations
// (docs/onboarding.md, "After the connect"). The pickers are Settings' own, from the same snapshot,
// so the step offers exactly what the account's page would. Compose-free, so the JVM suite
// reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkSlot

/** The calendar and address-book servers detection found beside a JMAP server, for [account]. */
internal data class Beside(
    val account: String,
    val email: String,
    val caldavUrl: String?,
    val carddavUrl: String?,
) {
    /** The host a person recognises the servers by. */
    val host: String get() = (caldavUrl ?: carddavUrl)?.let(::urlHost).orEmpty()
}

/** What the servers found beside a JMAP server would fill: only what its session lacks. */
internal enum class Fills {
    CALENDAR,
    CONTACTS,
    BOTH,
    ;

    companion object {
        /**
         * What [beside] can fill of what the account lacks; contacts are looked for at the
         * calendar's server when no address book was found (`dav_setup_beside`).
         */
        fun of(beside: Beside, calendar: Boolean, contacts: Boolean): Fills? {
            val fillsCalendar = calendar && beside.caldavUrl != null
            val fillsContacts = contacts && (beside.carddavUrl != null || beside.caldavUrl != null)
            return when {
                fillsCalendar && fillsContacts -> BOTH
                fillsCalendar -> CALENDAR
                fillsContacts -> CONTACTS
                else -> null
            }
        }
    }
}

/**
 * The step for one account: its pickers, what the person picked in each (a suggestion starts
 * picked, and continuing confirms it), and the servers found beside its own, offered as an account
 * of their own while its server lacks a calendar or contacts.
 */
internal data class SetupLinkStep(
    val account: String,
    val pickers: List<LinkPicker>,
    val picked: List<Int?> = pickers.map { it.suggested },
    val beside: Pair<Beside, Fills>? = null,
    // Whether the person has changed a pick, after which a late suggestion leaves the step alone.
    val touched: Boolean = false,
) {
    /** Records a pick. */
    fun pick(picker: Int, option: Int?): SetupLinkStep =
        if (picker !in picked.indices) {
            this
        } else {
            copy(picked = picked.toMutableList().also { it[picker] = option }, touched = true)
        }

    /** Picks [added], an account just added from this step, wherever it fits and nothing is. */
    fun adopt(added: String): SetupLinkStep {
        var changed = false
        val next = pickers.mapIndexed { index, picker ->
            val at = picker.options.indexOfFirst { it.id == added }
            if (picked[index] == null && at >= 0) {
                changed = true
                at
            } else {
                picked[index]
            }
        }
        return if (changed) copy(picked = next, touched = true) else this
    }

    /**
     * The same step read again, for a suggestion that arrived after it was drawn. A step the person
     * has already changed keeps what they picked, carried across by account, slot by slot; the
     * fresh pickers stand, since they may offer an account added since.
     */
    fun refreshed(fresh: SetupLinkStep): SetupLinkStep {
        if (!touched) return fresh
        val carried = fresh.pickers.mapIndexed { index, picker ->
            val earlier = pickers.indexOfFirst { it.slot == picker.slot }
            if (earlier < 0) {
                fresh.picked[index]
            } else {
                picked[earlier]
                    ?.let { pickers[earlier].options.getOrNull(it)?.id }
                    ?.let { id -> picker.options.indexOfFirst { it.id == id }.takeIf { at -> at >= 0 } }
            }
        }
        return fresh.copy(picked = carried, touched = true)
    }

    /** The links to set: each slot picked, and the account it names. */
    fun links(): List<Pair<LinkSlot, String>> = pickers.mapIndexedNotNull { index, picker ->
        picked[index]?.let { picker.options.getOrNull(it) }?.let { picker.slot to it.id }
    }

    companion object {
        /**
         * The step for [account], or null when no other account can fill anything it lacks and its
         * server offers its calendar and contacts, or its kind is not offered linking at setup
         * ([offersLinks], the core's `offers_setup_links`: a Microsoft or Google account holds
         * everything itself, and linking one is for Settings). A server without a calendar or
         * contacts gets the step anyway, for "Add another account" and for [beside], which is
         * offered only then.
         */
        fun of(
            snapshot: AccountsSnapshot,
            account: String,
            offersLinks: (CoreAccountKind) -> Boolean,
            beside: Beside? = null,
        ): SetupLinkStep? {
            val entry = snapshot.accounts.firstOrNull { it.id == account } ?: return null
            if (!offersLinks(entry.kind)) return null
            val pickers = AccountSettingsRules.linkPickers(entry).filter { it.selected == null }
            fun lacks(capability: AccountCapability) = entry.uses.any {
                it.capability == capability && it.state == CapabilityState.NOT_OFFERED
            }
            val calendar = lacks(AccountCapability.CALENDAR)
            val contacts = lacks(AccountCapability.CONTACTS)
            if (pickers.isEmpty() && !calendar && !contacts) return null
            val offered = beside?.let { found -> Fills.of(found, calendar, contacts)?.let { found to it } }
            return SetupLinkStep(account, pickers, beside = offered)
        }
    }
}
