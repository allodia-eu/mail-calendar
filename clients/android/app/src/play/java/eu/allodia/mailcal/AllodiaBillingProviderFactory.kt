// Which shop the `play` APK sells through.
//
// One of two files with this signature, one per flavour, which is how the flavour decides the shop
// without anything above it holding a branch for the two cases.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.AllodiaStore
import uniffi.mailcal_bindings.MailcalApp

internal fun allodiaBillingProvider(
    context: Context,
    app: MailcalApp,
    onPurchaseReported: () -> Unit,
): AllodiaBillingProvider =
    GooglePlayBillingProvider(
        context = context,
        catalogue = app.allodiaStoreProducts(store = AllodiaStore.GOOGLE),
        onPurchaseReported = onPurchaseReported,
        // Asked on every purchase rather than read once here, so signing in as somebody else
        // reaches the next purchase without the provider being rebuilt.
        accountId = { app.allodiaAccount()?.id },
    )
