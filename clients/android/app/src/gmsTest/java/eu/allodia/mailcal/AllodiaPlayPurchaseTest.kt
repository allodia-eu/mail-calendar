package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreProduct

// What a device decides about a Play purchase before the core sees it. Everything above this needs
// a BillingClient and a device; this is the half a JVM suite can hold.
class AllodiaPlayPurchaseTest {
    private val catalogue =
        listOf(
            AllodiaStoreProduct(
                plan = AllodiaPlan.YEARLY,
                productId = "eu.allodia.mailcal.services",
                basePlan = "yearly",
            ),
            AllodiaStoreProduct(
                plan = AllodiaPlan.MONTHLY,
                productId = "eu.allodia.mailcal.services",
                basePlan = "monthly",
            ),
        )

    private fun purchases(
        productIds: List<String> = listOf("eu.allodia.mailcal.services"),
        purchaseTimeMillis: Long = 1_760_000_000_000,
        isPurchased: Boolean = true,
    ) =
        allodiaPurchasesFrom(
            catalogue = catalogue,
            productIds = productIds,
            purchaseToken = "a-purchase-token",
            purchaseTimeMillis = purchaseTimeMillis,
            isPurchased = isPurchased,
        )

    // Play reports the purchase time in milliseconds and every clock in the core takes seconds.
    // Getting this wrong puts a 2026 purchase in 1970, which the core then reads as a purchase
    // that has been stuck since the epoch and tells the person so.
    @Test
    fun `the purchase time crosses in seconds, not milliseconds`() {
        assertEquals(1_760_000_000L, purchases().single().purchasedAt)
    }

    // The token is the whole of what the service takes: what it presents to the Play Developer API,
    // and what makes a retry attach once rather than once per attempt.
    @Test
    fun `the purchase token is what crosses, and it names the store that sold it`() {
        val purchase = purchases().single()

        assertEquals("a-purchase-token", purchase.purchase)
        assertEquals(AllodiaStore.GOOGLE, purchase.store)
    }

    // A cash or bank payment that has not cleared is money nobody has paid yet. Redeeming one asks
    // the service to grant a subscription that does not exist; Play reports it again when it
    // clears, which is when it becomes real.
    @Test
    fun `a pending payment is not carried over`() {
        assertTrue(purchases(isPurchased = false).isEmpty())
    }

    // A purchase for something this build never published belongs to some other product. Handing it
    // over would ask the service to verify a purchase it was never told about.
    @Test
    fun `a purchase of something else is ignored`() {
        assertTrue(purchases(productIds = listOf("eu.allodia.something.else")).isEmpty())
    }
}
