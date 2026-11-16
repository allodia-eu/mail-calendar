// Signing in to an Allodia account, which the core runs (docs/waiting.md): a press sends an
// AllodiaIntent, the browser sign-in driver opens the sign-in and hands back its redirect as it does
// an account's (MainActivityBrowserSignIn.kt), and both cards are drawn from the core's
// `allodiaCard()`, the subscription section from `allodiaSubscriptionCard()`. What is left here is
// the core's signal and the service's own pages: the one a sign-out names and Manage account.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AllodiaCard
import uniffi.mailcal_bindings.AllodiaCardState
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaOnboarding
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.allodiaSignInAvailable

// Whether this build carries the route at all. False is the ordinary answer for a build from
// source, and the settings card then draws nothing rather than a button that dead-ends.
internal fun allodiaSignInOffered(): Boolean = allodiaSignInAvailable()

/** What the cards draw before the core has said anything: nobody signed in, nothing running. */
internal val ALLODIA_SIGNED_OUT = ControlState(enabled = true, running = false).let { free ->
    AllodiaCard(
        state = AllodiaCardState.SignedOut,
        browser = null,
        signIn = free,
        create = free,
        signOut = free,
        failure = null,
        endSession = null,
        onboarding = AllodiaOnboarding.Offering,
    )
}

/** The sign-in, or with [create] creating an account first, returning to this app's redirect. */
internal fun allodiaSignIn(create: Boolean = false): AllodiaIntent =
    AllodiaIntent.SignIn(create, AllodiaOAuthConfig.REDIRECT_URI)

internal fun MainActivity.changeAllodia(intent: AllodiaIntent) {
    app?.dispatch(Intent.Allodia(intent))
}

// Opens the page where someone changes their details or deletes the account. Nothing is pending
// and nothing comes back: this is a page, not a flow.
internal fun MainActivity.manageAllodiaAccount() {
    val url = app?.allodiaAccountUrl() ?: return
    logUiInfo("allodia: opening the account page")
    openAllodiaAccountPage(this, url)
}

/** The cards at connect, read without acting on them: a page the core named before is not new. */
internal fun MainActivity.seeAllodia(instance: MailcalApp) {
    allodiaCard = instance.allodiaCard()
    allodiaEndSessionOpened = allodiaCard.endSession?.id ?: 0uL
    allodiaSubscription = instance.allodiaSubscriptionCard()
    allodiaCheckoutOpened = allodiaSubscription.checkoutPage?.id ?: 0uL
}

/**
 * The core's Allodia signal: the cards read their card again, and the service's sign-out page and
 * the subscription's payment page each open once.
 */
internal fun MainActivity.allodiaSignalled(instance: MailcalApp) {
    val card = instance.allodiaCard()
    allodiaCard = card
    card.endSession?.takeIf { it.id > allodiaEndSessionOpened }?.let { end ->
        allodiaEndSessionOpened = end.id
        // Best-effort and unreported: this device is signed out whatever the browser does. What it
        // buys is the next sign-in asking who you are rather than completing silently against a
        // session someone thought they had left.
        logUiInfo("allodia: ending the browser session")
        runCatching { openAllodiaAccountPage(this, end.url) }
    }
    allodiaSubscription = instance.allodiaSubscriptionCard()
    openAllodiaCheckoutPage()
}
