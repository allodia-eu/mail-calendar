// A calendar or address book that did not connect on an account that did, said above what the
// calendar or contacts screen shows of it (docs/accounts.md rule 16). The reason is the sentence
// the account's own page says beneath the use's switch, so the two never word one problem twice.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.ConnectivitySnapshot
import uniffi.mailcal_bindings.UseOutage
import uniffi.mailcal_bindings.UseProblem

/** The screen a line is drawn on. */
internal enum class OutageUse(val capability: AccountCapability) {
    CALENDAR(AccountCapability.CALENDAR),
    CONTACTS(AccountCapability.CONTACTS),
}

/** One line above the week or the list: the account it names, whose page its action opens. */
internal data class OutageLine(val accountId: String, val text: String)

internal object UseOutageRules {
    /** Title, reason and "last synced", in that order, as one line. */
    fun text(ctx: Context, use: OutageUse, address: String, problem: UseProblem): String {
        val title = when (use) {
            OutageUse.CALENDAR -> L10n.calendar_outage_title(ctx, address)
            OutageUse.CONTACTS -> L10n.contacts_outage_title(ctx, address)
        }
        val reason = useNoteText(ctx, problemNote(use.capability, problem))
        return listOfNotNull(title, reason, L10n.use_outage_last_synced(ctx)).joinToString(" ")
    }

    /**
     * One line per outage of [use] in [snapshot], in its order; none before the first snapshot.
     * [addressOf] names an account by its address. The core has already left out what this screen
     * should not say.
     */
    fun lines(
        ctx: Context,
        use: OutageUse,
        snapshot: ConnectivitySnapshot?,
        addressOf: (String) -> String,
    ): List<OutageLine> {
        val outages: List<UseOutage> = when (use) {
            OutageUse.CALENDAR -> snapshot?.calendarOutages
            OutageUse.CONTACTS -> snapshot?.contactsOutages
        }.orEmpty()
        return outages.map { outage ->
            val address = addressOf(outage.accountId)
            OutageLine(outage.accountId, text(ctx, use, address, outage.problem))
        }
    }
}
