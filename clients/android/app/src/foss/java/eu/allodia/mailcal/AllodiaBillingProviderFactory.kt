// Which shop the `foss` APK sells through.
//
// One of two files with this signature, one per flavour. Allodia's own checkout is the only shop
// here, and there is no store to report a purchase this app did not ask for, so
// `onPurchaseReported` is never called: a renewal or a payment clearing happens at the service,
// and the account already carries it.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.MailcalApp

internal fun allodiaBillingProvider(
    context: Context,
    app: MailcalApp,
    @Suppress("UNUSED_PARAMETER") onPurchaseReported: () -> Unit,
): AllodiaBillingProvider = WebBillingProvider(context = context, app = app)
