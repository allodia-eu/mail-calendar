// Detection answers for the setup suites, shaped as the core shapes them, without the cdylib.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.DetectedSetup
import uniffi.mailcal_bindings.SetupChoice
import uniffi.mailcal_bindings.SetupRecommendation

internal fun choice(capability: AccountCapability, on: Boolean, found: Boolean = on) =
    SetupChoice(capability, on, found)

/**
 * [recommendation] with the choices the core offers a standards account: mail when it has a mail
 * server, then calendar and contacts, each on when its server is known, contacts looked for at the
 * calendar's server when no address book was found (`standards_choices`).
 */
internal fun standardsSetup(
    recommendation: SetupRecommendation,
    caldav: String? = (recommendation as? SetupRecommendation.Imap)?.caldavUrl,
    carddav: String? = null,
    calendarAndContacts: Boolean = false,
): DetectedSetup {
    if (recommendation !is SetupRecommendation.Imap && !calendarAndContacts) return detected(recommendation)
    val choices = listOfNotNull(
        choice(AccountCapability.MAIL, true).takeIf { !calendarAndContacts },
        choice(AccountCapability.CALENDAR, caldav != null),
        choice(AccountCapability.CONTACTS, caldav != null || carddav != null),
    )
    return DetectedSetup(recommendation, calendarAndContacts, caldav, carddav, choices)
}
