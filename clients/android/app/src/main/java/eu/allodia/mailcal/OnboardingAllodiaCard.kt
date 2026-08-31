// First run: the Allodia-account recommendation, above the address field.
//
// docs/onboarding.md is the contract and decides the order, the card, the way back for someone who
// already has one, a divider naming what follows, then the address field. Its Apple, Windows and
// Linux twins draw the same four things in the same order.
//
// Three rules it is easy to break silently:
//
//   * A build with no Allodia registration loses the card, the sign-in line AND the divider
//     together. A lone "or connect directly" heading under nothing is the tell that the wrong
//     thing was gated.
//   * The copy may not out-run the capability matrix: phone and computer, never web.
//   * The card claims the account LIST and nothing else, never the mail, never a password.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AllodiaAccountOffer

/**
 * Everything the first-run card needs, gathered by the activity rather than the screen.
 *
 * The sign-in leaves for the browser and returns through `onNewIntent`, so nothing this screen
 * remembered would still be there when it does.
 */
internal data class OnboardingAllodia(
    val offered: Boolean,
    val signingIn: Boolean,
    // Whether the hop has outlasted its threshold, so the busy row draws the way back.
    val escapable: Boolean,
    val signedIn: Boolean,
    val checking: Boolean,
    // What the last pass answered with, or null while none has answered. `Some empty` is "this
    // account has no mail accounts"; null is "we have not looked", and only the first may say so.
    val offers: List<AllodiaAccountOffer>?,
    val onCreate: () -> Unit,
    val onSignIn: () -> Unit,
    val onCancelSignIn: () -> Unit,
    // Whether this is the screen somebody cannot skip. The card is asked once; the offers are not.
    val firstRun: Boolean,
)

/**
 * The card, the sign-in line and the divider, or nothing at all.
 *
 * [offered] is `allodiaSignInAvailable()`, asked once by the caller. Nothing here reads a
 * credential itself, which is the single-question rule in the contract.
 *
 * Once somebody has signed in, the card is replaced by what their other devices hold: the whole
 * reason to sign in here is that the next screen should not be an empty address field.
 */
@Composable
internal fun OnboardingAllodiaCard(
    offered: Boolean,
    signingIn: Boolean,
    signedIn: Boolean,
    offers: List<AllodiaAccountOffer>?,
    checking: Boolean,
    onCreate: () -> Unit,
    onSignIn: () -> Unit,
    onSetUp: (AllodiaAccountOffer) -> Unit,
    escapable: Boolean = false,
    onCancelSignIn: () -> Unit = {},
    firstRun: Boolean = true,
) {
    if (!offered) return
    val ctx = LocalContext.current
    // What an offer is, and what the card is, part company on the second account. The card is a
    // pitch and is asked once: somebody who signed in has decided. The offers are not a pitch --
    // they are accounts they already have, and hiding them behind "you have decided" is what made
    // the second of three linked accounts reachable only from a Settings page.
    if (!firstRun) {
        if (!offers.isNullOrEmpty()) {
            OfferRows(offers, onSetUp)
            OnboardingDivider()
        }
        return
    }
    when {
        signingIn || (signedIn && checking) -> Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            CircularProgressIndicator()
            Text(
                if (signingIn) {
                    L10n.settings_allodia_signing_in(ctx)
                } else {
                    L10n.settings_allodia_sync_checking(ctx)
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            // The way out of a hop that did not come back. Only for the browser leg: the pass
            // below it is a bounded network call, not a wait on somebody in another application.
            if (signingIn && escapable) {
                TextButton(onClick = onCancelSignIn) { Text(L10n.action_cancel(ctx)) }
            }
        }

        signedIn -> {
            // Signed in and asked. Offers become the fast route; none means this account has no
            // mail accounts on it yet -- which is said in words, because the alternative is a
            // divider over an address field with the card gone, and that reads as a failed
            // sign-in rather than an empty answer (docs/onboarding.md).
            // A pass that has not answered -- one that failed on the network -- says nothing.
            // Reporting it as an empty account states a result nobody has.
            if (offers == null) {
                Unit
            } else if (offers.isEmpty()) {
                Text(
                    L10n.setup_allodia_none_title(ctx),
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    L10n.setup_allodia_none_body(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                OfferRows(offers, onSetUp)
            }
        }

        else -> {
            // One control, not a heading beside a button: a screen reader announces the offer and
            // its action together. The label carries the ACTION, never the "Recommended" marker.
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .semantics { contentDescription = L10n.setup_allodia_title(ctx) },
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Text(
                        L10n.setup_allodia_recommended(ctx),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Text(
                        L10n.setup_allodia_title(ctx),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        L10n.setup_allodia_subtitle(ctx),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    TextButton(onClick = onCreate) {
                        Text(L10n.setup_allodia_create(ctx))
                    }
                }
            }
            // One line, not a second card of equal weight.
            TextButton(onClick = onSignIn) {
                Text(L10n.setup_allodia_have_one(ctx))
            }
        }
    }
    OnboardingDivider()
}

/// The accounts the person's other devices hold. The button carries the whole record, not the
/// address: the route comes from what the other device wrote down, which is the point of having
/// synced it.
@Composable
private fun OfferRows(offers: List<AllodiaAccountOffer>, onSetUp: (AllodiaAccountOffer) -> Unit) {
    val ctx = LocalContext.current
    Text(
        L10n.settings_allodia_sync_heading(ctx),
        style = MaterialTheme.typography.titleMedium,
    )
    offers.forEach { offer ->
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(offer.email, style = MaterialTheme.typography.bodyMedium)
            TextButton(onClick = { onSetUp(offer) }) {
                Text(L10n.settings_allodia_sync_set_up(ctx))
            }
        }
    }
}

/// What the address field below is, named. Only ever under something: a lone "or connect directly"
/// heading over nothing is the tell that a client gated the wrong half.
@Composable
private fun OnboardingDivider() {
    val ctx = LocalContext.current
    HorizontalDivider()
    Text(
        L10n.setup_allodia_divider(ctx),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
