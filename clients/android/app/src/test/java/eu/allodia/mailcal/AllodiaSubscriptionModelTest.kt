// The sentences a subscription answer decides, before anything draws them: who is charging,
// whether it will charge again, whether a failed charge is being retried, and what this device may
// buy at all.
//
// Each of these is silent when wrong. A renewing subscription described as running out, a retry
// drawn as a lapse, or a store offer put in front of a device that cannot tag the purchase all
// render perfectly and say the wrong thing.
package eu.allodia.mailcal

import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaOwnStatus
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreStatus

private fun offer(store: AllodiaStore, plan: AllodiaPlan = AllodiaPlan.YEARLY) =
    AllodiaOffer(plan = plan, store = store, displayPrice = "€29.90")

class AllodiaSubscriptionModelTest {
    /** Allodia's own billing first, then each store in the order the service listed them. */
    @Test
    fun every_biller_charging_the_account_is_named_in_a_stable_order() {
        val both = subscription(
            entitled = true,
            own = ownSubscription(),
            stores = listOf(storeSubscription(source = AllodiaStore.GOOGLE)),
        )
        assertEquals(
            listOf(AllodiaBiller.Allodia, AllodiaBiller.Google),
            allodiaBillers(both),
        )
    }

    /**
     * A biller this build cannot name still appears.
     *
     * Dropping it would turn "two things are charging you" into a sentence naming one, which is
     * the half the person can act on missing.
     */
    @Test
    fun a_biller_this_build_cannot_name_is_still_named() {
        assertEquals("Vendor X", allodiaBillerName(AllodiaBiller.Unknown("Vendor X")))
    }

    /** A store that renews says so itself; Allodia's own says so by still having a next payment. */
    @Test
    fun a_subscription_renews_when_anything_will_charge_again() {
        assertTrue(allodiaWillRenew(subscription(stores = listOf(storeSubscription()))))
        assertFalse(
            allodiaWillRenew(subscription(stores = listOf(storeSubscription(autoRenewing = false))))
        )
        assertTrue(allodiaWillRenew(subscription(own = ownSubscription())))
        assertFalse(
            allodiaWillRenew(subscription(own = ownSubscription(nextPaymentDate = null)))
        )
    }

    /**
     * A cancelled subscription that is still paid up runs until its date; it does not renew.
     *
     * The two sentences differ by one word and the wrong one tells somebody their access is safe
     * when it is about to stop.
     */
    @Test
    fun a_cancelled_subscription_runs_until_its_date_rather_than_renewing() {
        val cancelled = subscription(
            entitled = true,
            own = ownSubscription(status = AllodiaOwnStatus.Cancelled),
        )
        assertFalse(allodiaWillRenew(cancelled))
    }

    /** A retried charge is not a lapse: access continues, and the wording has to reflect that. */
    @Test
    fun a_failed_charge_being_retried_is_grace_and_not_a_lapse() {
        assertTrue(
            allodiaInGrace(
                subscription(stores = listOf(storeSubscription(status = AllodiaStoreStatus.Grace)))
            )
        )
        assertTrue(
            allodiaInGrace(subscription(own = ownSubscription(status = AllodiaOwnStatus.PastDue)))
        )
        assertFalse(allodiaInGrace(subscription(stores = listOf(storeSubscription()))))
    }

    /**
     * A device that cannot name its Allodia account may not buy from a store.
     *
     * The id a purchase is tagged with is the only way to attribute one whose report never
     * arrives, and it cannot be added afterwards, so the offer is withheld rather than taken up
     * into a purchase nobody can repair. Allodia's own checkout needs no tag and stays.
     */
    @Test
    fun a_device_with_no_account_id_is_offered_the_checkout_and_no_store() {
        val offers = listOf(offer(AllodiaStore.GOOGLE), offer(AllodiaStore.ALLODIA))

        assertEquals(offers, allodiaBuyableOffers(offers, accountId = "an-id"))
        assertEquals(
            listOf(offer(AllodiaStore.ALLODIA)),
            allodiaBuyableOffers(offers, accountId = null),
        )
        assertTrue(allodiaNeedsReauthToBuy(offers, accountId = null))
        assertFalse(allodiaNeedsReauthToBuy(offers, accountId = "an-id"))
    }

    /**
     * A build whose only shop is Allodia's own checkout never asks anybody to sign in again.
     *
     * Nothing about that route needs the id, so the prompt would be an instruction to fix
     * something that is not broken.
     */
    @Test
    fun the_checkout_route_alone_never_asks_for_a_fresh_sign_in() {
        val offers = listOf(offer(AllodiaStore.ALLODIA))
        assertFalse(allodiaNeedsReauthToBuy(offers, accountId = null))
    }

    /**
     * ⚠️ The account service is a different server from the sync engine and sends a numeric offset
     * rather than the engine's `Z`. A reader parsing only the engine's shape drew every date on
     * this card as a raw `2026-09-18`.
     */
    @Test
    fun a_date_from_the_account_service_is_localised_whatever_shape_it_arrives_in() {
        for (raw in listOf("2026-09-18T00:00:00+02:00", "2026-09-17T23:00:00Z", "2026-09-18")) {
            val dutch = allodiaDate(raw, Locale.forLanguageTag("nl-NL"))
            assertEquals("september", dutch?.substringAfter(' ')?.substringBefore(' '))
            assertTrue(raw, dutch!!.contains("2026"))
        }
    }

    /** An empty string is no date at all, and a shape nothing can read still has the right day. */
    @Test
    fun an_unreadable_date_keeps_its_day_and_only_stops_being_localised() {
        assertNull(allodiaDate("", Locale.UK))
        assertEquals("18/09/2026", allodiaDate("18/09/2026 ish", Locale.UK))
    }
}
