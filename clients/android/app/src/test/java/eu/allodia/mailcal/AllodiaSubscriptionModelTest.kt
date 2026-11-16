// The words and formats the subscription section puts around the core's card, and what a device
// that buys from a store decides before the core sees it.
//
// Who is billing, whether it renews and which writes exist are the core's, and tested there.
// What is left here is silent when wrong: a store offer put in front of a device that cannot tag
// the purchase, or a date drawn in the wire's shape, render perfectly and say the wrong thing.
package eu.allodia.mailcal

import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AllodiaBiller
import uniffi.mailcal_bindings.AllodiaOffer
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.ControlState

private fun offer(store: AllodiaStore, plan: AllodiaPlan = AllodiaPlan.YEARLY) =
    AllodiaOffer(plan = plan, store = store, displayPrice = "€29.90")

class AllodiaSubscriptionModelTest {
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
     * ⚠️ A currency this JVM cannot name is still an amount worth showing.
     *
     * The service sends minor units and a code, and the code arrives empty when the read that
     * carries it did not. Throwing there would take down a screen over a symbol.
     */
    @Test
    fun an_amount_survives_a_currency_this_machine_cannot_name() {
        val dutch = Locale.forLanguageTag("nl-NL")
        assertTrue("2,99", allodiaMinorUnits(299, "EUR", dutch).contains("2,99"))
        assertTrue("no code", allodiaMinorUnits(299, "", dutch).contains("2,99"))
        assertTrue("nonsense code", allodiaMinorUnits(299, "not-a-currency", dutch).contains("2,99"))
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

    /**
     * Allodia's checkout is the core's write, so its button carries the card's state and is gone
     * once the card stops selling it; a store's purchase is this device's, held while a sheet is
     * up and running on the period being bought.
     */
    @Test
    fun an_offer_is_drawn_with_the_state_of_whoever_runs_its_purchase() {
        val running = ControlState(enabled = false, running = true)
        val held = ControlState(enabled = false, running = false)
        val checkout = allodiaCheckout(monthly = held, yearly = running)

        assertEquals(running, allodiaOfferControl(offer(AllodiaStore.ALLODIA), checkout, buying = null))
        assertEquals(
            held,
            allodiaOfferControl(offer(AllodiaStore.ALLODIA, AllodiaPlan.MONTHLY), checkout, buying = null),
        )
        assertNull(allodiaOfferControl(offer(AllodiaStore.ALLODIA), checkout = null, buying = null))

        assertEquals(
            ControlState(enabled = true, running = false),
            allodiaOfferControl(offer(AllodiaStore.GOOGLE), checkout = null, buying = null),
        )
        assertEquals(running, allodiaOfferControl(offer(AllodiaStore.GOOGLE), null, AllodiaPlan.YEARLY))
        assertEquals(
            held,
            allodiaOfferControl(offer(AllodiaStore.GOOGLE, AllodiaPlan.MONTHLY), null, AllodiaPlan.YEARLY),
        )
    }
}
