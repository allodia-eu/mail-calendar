// Keeping this device's account list in step with the person's other devices. The core runs every
// pass and keeps what it found (`allodia_devices`); what is here is handing it somewhere to keep its
// bookkeeping, and what the first-run card makes of its answer.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaDevices
import uniffi.mailcal_bindings.MailcalApp

/**
 * Hand the core somewhere to remember what it has synced, which is what lets it run a pass: one
 * starts here when somebody is signed in. Answers whether it was installed.
 *
 * Called on the connect thread, and never in a dev-account launch, whose canned harness accounts
 * must not reach the person's real account list.
 */
internal fun MainActivity.installAllodiaSyncStore(instance: MailcalApp): Boolean =
    try {
        instance.useAllodiaSyncStateStore(
            SharedPrefsSyncStateStore(this, SYNC_STATE_FILE),
        )
        true
    } catch (e: Exception) {
        // The blob could not be read. Syncing is off for this launch rather than starting from
        // nothing, which would re-adopt every record and re-offer every account.
        logUiWarn("allodia: the sync state could not be read (${e.message}); not syncing this launch")
        false
    }

/**
 * The offers the first-run card draws: null until a pass has answered, and while the last answer
 * holds no offers but is not an empty one (questions only, or a failure), since only an answer
 * that found nothing may say the account holds none (docs/onboarding.md).
 */
internal fun onboardingOffers(devices: AllodiaDevices?): List<AllodiaAccountOffer>? = when {
    devices == null -> null
    devices.nothingFound -> emptyList()
    devices.offers.isEmpty() -> null
    else -> devices.offers
}
