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
     * ⚠️ A store that has stopped charging is not a biller, whatever order the service listed it
     * in.
     *
     * Observed on a device: the account had bought through the App Store hours earlier, that
     * sandbox subscription had since expired, and a purchase through Google Play was then told
     * "Billed by Apple". The service was right both times; the reading of it was not. The expired
     * store keeps its manage button, so this is about the sentence, not about hiding the row.
     */
    @Test
    fun a_store_that_has_stopped_charging_is_not_who_is_billing_you() {
        val lapsedApple = subscription(
            entitled = true,
            stores = listOf(
                storeSubscription(
                    source = AllodiaStore.APPLE,
                    status = AllodiaStoreStatus.Expired,
                    autoRenewing = false,
                ),
                storeSubscription(source = AllodiaStore.GOOGLE),
            ),
        )
        assertEquals(listOf(AllodiaBiller.Google), allodiaBillers(lapsedApple))
    }

    /**
     * Cancelled is still being billed in the only sense that matters here: the period was paid for
     * and is still running. Revoked, on hold and paused grant nothing and name nobody.
     */
    @Test
    fun a_cancelled_subscription_still_names_its_store_and_a_revoked_one_does_not() {
        for (status in listOf(AllodiaStoreStatus.Cancelled, AllodiaStoreStatus.Grace)) {
            val live = subscription(stores = listOf(storeSubscription(status = status)))
            assertEquals(status.toString(), listOf(AllodiaBiller.Google), allodiaBillers(live))
        }
        for (status in listOf(
            AllodiaStoreStatus.Revoked,
            AllodiaStoreStatus.OnHold,
            AllodiaStoreStatus.Paused,
            AllodiaStoreStatus.Unknown("something new"),
        )) {
            val dead = subscription(stores = listOf(storeSubscription(status = status)))
            assertEquals(status.toString(), emptyList<AllodiaBiller>(), allodiaBillers(dead))
        }
    }

    /**
     * ⚠️ "Who is billing you" and "is there anything to do at the store" are different questions,
     * and they part company on the two states where somebody most needs the answer.
     *
     * On hold and paused grant nothing, so neither may say it is billing you; both are fixed only
     * at the store, so both keep the way there. Expired and revoked are the other way about:
     * nothing to manage, and offering the route walks somebody into the store's own resubscribe
     * button while another source is already charging them.
     */
    @Test
    fun a_lapsed_store_offers_no_way_in_but_a_held_one_does() {
        for (status in listOf(AllodiaStoreStatus.OnHold, AllodiaStoreStatus.Paused)) {
            assertFalse(status.toString(), allodiaStoreIsBilling(status))
            assertTrue(status.toString(), allodiaStoreCanBeManaged(status))
        }
        for (status in listOf(AllodiaStoreStatus.Expired, AllodiaStoreStatus.Revoked)) {
            assertFalse(status.toString(), allodiaStoreIsBilling(status))
            assertFalse(status.toString(), allodiaStoreCanBeManaged(status))
        }
        for (status in listOf(
            AllodiaStoreStatus.Active,
            AllodiaStoreStatus.Grace,
            AllodiaStoreStatus.Cancelled,
        )) {
            assertTrue(status.toString(), allodiaStoreIsBilling(status))
            assertTrue(status.toString(), allodiaStoreCanBeManaged(status))
        }
    }

    /** A checkout that was started and never paid is not somebody who is being charged. */
    @Test
    fun a_checkout_awaiting_its_first_payment_names_nobody() {
        val pending = subscription(
            own = ownSubscription(status = AllodiaOwnStatus.PendingFirstPayment)
        )
        assertEquals(emptyList<AllodiaBiller>(), allodiaBillers(pending))
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
     * A switch moves to the other period, and only where the service said it may.
     *
     * ⚠️ **`actions` is the authority, not the subscription's shape.** It was computed with every
     * biller in view, which a client cannot do: somebody the App Store is charging has a period,
     * and switching it here is exactly what the service refuses.
     */
    @Test
    fun a_switch_offers_the_other_period_and_only_where_it_is_allowed() {
        val monthly = subscription(entitled = true, own = ownSubscription(interval = AllodiaPlan.MONTHLY))
        assertEquals(AllodiaPlan.YEARLY, allodiaSwitchTarget(monthly))

        val yearly = subscription(entitled = true, own = ownSubscription(interval = AllodiaPlan.YEARLY))
        assertEquals(AllodiaPlan.MONTHLY, allodiaSwitchTarget(yearly))

        assertNull("no subscription of Allodia's own", allodiaSwitchTarget(subscription(entitled = true)))
        assertNull("the service refused", allodiaSwitchTarget(monthly.copy(actions = refusing())))
    }

    /**
     * Restarting is offered for a cancelled subscription and nothing else.
     *
     * A subscription still running has nothing to restart, and one whose period has run out is a
     * fresh checkout rather than a restart, which is a distinction only the service can make.
     */
    @Test
    fun restarting_is_offered_only_for_a_cancelled_subscription() {
        val cancelled = subscription(
            entitled = true,
            own = ownSubscription(status = AllodiaOwnStatus.Cancelled, nextPaymentDate = null),
        )
        assertTrue(allodiaOffersRestart(cancelled))
        assertFalse(allodiaOffersRestart(subscription(entitled = true, own = ownSubscription())))
        assertFalse(allodiaOffersRestart(cancelled.copy(actions = refusing())))
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
}
