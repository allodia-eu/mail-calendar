// Turning what Play reports into what the core attaches to the account.
//
// Its own file, and a plain function, because everything above it needs a `BillingClient` and a
// device: this is the part that can be asserted on the JVM suite (`./gradlew :app:test`), and it is
// where the two decisions a device actually makes here live.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AllodiaStorePurchase
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.AllodiaStoreProduct

// What the core is told about one Play purchase.
//
// Two rules, and both cost money to get wrong:
//
//  - **A purchase that is not PURCHASED is not carried over.** Play reports a PENDING purchase
//    (a cash or bank payment not yet cleared) alongside the settled ones, and nothing has been paid
//    for it yet. Attaching one would ask the service to grant a subscription nobody has bought;
//    Play reports it again once it clears, which is when it becomes real.
//  - **A purchase naming a product this build did not publish is ignored.** It belongs to another
//    of the app's products, if there ever is one, and handing it over would be asking the service
//    to verify something it was never told about.
//
// The purchase time arrives in **milliseconds** from Play and leaves in seconds, which is the unit
// every clock in the core takes. Getting that wrong puts a 2026 purchase in 1970, which reads as an
// hour-old purchase that has been stuck since the epoch.
internal fun allodiaPurchasesFrom(
    catalogue: List<AllodiaStoreProduct>,
    productIds: List<String>,
    purchaseToken: String,
    purchaseTimeMillis: Long,
    isPurchased: Boolean,
): List<AllodiaStorePurchase> {
    if (!isPurchased) return emptyList()
    val published = catalogue.map { it.productId }.toSet()
    if (productIds.none { it in published }) return emptyList()
    return listOf(
        AllodiaStorePurchase(
            store = AllodiaStore.GOOGLE,
            // Play's purchase token is the whole of what the service takes: it is what the service
            // presents to the Play Developer API, and what makes a retry attach once rather than
            // once per attempt.
            purchase = purchaseToken,
            purchasedAt = purchaseTimeMillis / 1000,
        )
    )
}
