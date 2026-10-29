// What a setup card offers an account for, and what the person's choice makes of it
// (docs/account-autodetect.md rule 8, docs/accounts.md rules 2 and 10). Which uses are offered, and
// how each starts, is the core's (`detect_account_setup`, `provider_setup_choices`); this file turns
// the toggles on screen into the uses and servers the account is set up with. Compose-free, so the
// JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.SetupChoice

/**
 * The choices the core offered, in its order, and the servers detection found beside the route;
 * an empty URL is one it did not find.
 */
internal data class UseOffer(
    val choices: List<SetupChoice>,
    val caldavUrl: String = "",
    val carddavUrl: String = "",
) {
    /** The choice the core offered for [capability], if any. */
    fun choiceFor(capability: AccountCapability): SetupChoice? =
        choices.firstOrNull { it.capability == capability }

    companion object {
        val NONE = UseOffer(emptyList())

        /** The offer a detection run answered with. */
        fun of(setup: DetectedSetup) =
            UseOffer(setup.choices, setup.caldavUrl.orEmpty(), setup.carddavUrl.orEmpty())
    }
}

/** A calendar or contacts toggle as the person left it, and the server typed beneath it. */
internal data class PickedUse(val on: Boolean, val typed: String = "")

/** What the person chose, as the account is set up with it; [uses] null when nothing was offered. */
internal data class ChosenUses(
    val uses: List<AccountCapability>?,
    val caldavUrl: String,
    val carddavUrl: String,
)

/** Which typed server a choice still needs before the account can be set up. */
internal enum class MissingServer { NONE, CALENDAR, CONTACTS }

internal object SetupUses {
    /**
     * The account a card's toggles describe. A calendar stores the endpoint detection found, or the
     * one typed in its place. Contacts store an address book found apart from the calendar, or the
     * one typed. Contacts found only through the calendar's server keep that server even with the
     * calendar off, because that is where they are looked for, and the stored uses keep the
     * calendar closed. Colleagues count only beside contacts, and shared mailboxes only beside mail.
     */
    fun chosen(
        offer: UseOffer,
        mail: Boolean?,
        calendar: PickedUse?,
        contacts: PickedUse?,
        colleagues: Boolean?,
        sharedMailboxes: Boolean? = null,
    ): ChosenUses {
        val calendarOn = calendar?.on == true
        val contactsOn = contacts?.on == true
        fun foundOrTyped(found: String, picked: PickedUse?) =
            found.ifEmpty { picked?.typed?.trim().orEmpty() }
        val carddav = if (contactsOn && (offer.carddavUrl.isNotEmpty() || offer.caldavUrl.isEmpty())) {
            foundOrTyped(offer.carddavUrl, contacts)
        } else {
            ""
        }
        val throughCalendar = contactsOn && carddav.isEmpty() && offer.caldavUrl.isNotEmpty()
        val caldav = if (calendarOn || throughCalendar) foundOrTyped(offer.caldavUrl, calendar) else ""
        val uses = if (offer.choices.isEmpty()) {
            null
        } else {
            buildList<AccountCapability> {
                if (mail == true) {
                    add(AccountCapability.MAIL)
                    if (sharedMailboxes == true) add(AccountCapability.SHARED_MAILBOXES)
                }
                if (calendarOn) add(AccountCapability.CALENDAR)
                if (contactsOn) {
                    add(AccountCapability.CONTACTS)
                    if (colleagues == true) add(AccountCapability.COLLEAGUES)
                }
            }
        }
        return ChosenUses(uses, caldav, carddav)
    }

    /**
     * The server still to be typed for a use switched on: the calendar's, or the address book's
     * when no calendar gives contacts a server to be looked for at.
     */
    fun missing(offer: UseOffer, calendar: PickedUse?, contacts: PickedUse?): MissingServer {
        fun empty(choice: SetupChoice?, picked: PickedUse?) =
            choice?.serverFound == false && picked?.on == true && picked.typed.isBlank()
        if (empty(offer.choiceFor(AccountCapability.CALENDAR), calendar)) return MissingServer.CALENDAR
        return if (calendar?.on != true && empty(offer.choiceFor(AccountCapability.CONTACTS), contacts)) {
            MissingServer.CONTACTS
        } else {
            MissingServer.NONE
        }
    }

    /**
     * What a provider sign-in asks for: the uses chosen on screen, less any the address it signs in
     * with is not offered, so a personal address is never asked for colleagues.
     */
    fun allowed(chosen: List<AccountCapability>?, offered: List<SetupChoice>): List<AccountCapability>? =
        chosen?.filter { capability -> offered.any { it.capability == capability } }

    /**
     * What the servers typed on the manual calendar-and-contacts form make the account used for: its
     * calendar beside a calendar server, and contacts beside either, since they are looked for at
     * the calendar's server when no address book is given. Null when neither is typed.
     */
    fun davUses(caldav: String, carddav: String): List<AccountCapability>? = when {
        caldav.isNotBlank() -> listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)
        carddav.isNotBlank() -> listOf(AccountCapability.CONTACTS)
        else -> null
    }
}
