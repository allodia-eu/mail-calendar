// The three things somebody may do to the subscription Allodia bills directly: stop the recurring
// charge, start it again, and move between the two periods.
//
// ⚠️ **Only Allodia's own subscription, never a store's.** A store's is changed at that store and
// nowhere else, so the card offers `manageUrl` for those and none of this. What decides whether a
// button exists at all is `actions`, which the service computed with every biller in view; a client
// is in no position to work that out and does not try.
//
// The fourth write, starting a checkout, is not here: it is what `WebBillingProvider` already calls
// behind the ordinary buy buttons, because on the `foss` flavour Allodia's own checkout *is* the
// shop (`purchasing.md`, "The shop follows the channel").
package eu.allodia.mailcal

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import java.util.Locale
import uniffi.mailcal_bindings.AllodiaOwnStatus
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaSubscription

// Which confirmation is on screen, if any.
//
// Both of these change what somebody is charged, and neither says so anywhere else: a cancellation
// is silent until the period runs out, and a switch is silent until a date that may be a month
// away. So both are asked before they are done.
private enum class AllodiaPendingWrite {
    CANCEL,
    SWITCH,
}

// The period a switch would move to, which is simply the other one.
//
// Null when there is nothing to switch: no subscription of Allodia's own, or a service that has
// said this account may not switch.
internal fun allodiaSwitchTarget(subscription: AllodiaSubscription): AllodiaPlan? {
    if (!subscription.actions.canSwitchInterval) return null
    return when (subscription.own?.interval) {
        AllodiaPlan.MONTHLY -> AllodiaPlan.YEARLY
        AllodiaPlan.YEARLY -> AllodiaPlan.MONTHLY
        null -> null
    }
}

// Whether to offer starting the subscription again, which is a different question from whether it
// has been cancelled: the service decides, because a cancelled subscription whose period has run
// out is a new checkout rather than a restart, and only it knows which.
internal fun allodiaOffersRestart(subscription: AllodiaSubscription): Boolean =
    subscription.actions.canResubscribe && subscription.own?.status == AllodiaOwnStatus.Cancelled

@Composable
internal fun AllodiaSubscriptionWrites(
    subscription: AllodiaSubscription,
    onCancel: () -> Unit,
    onResubscribe: () -> Unit,
    onSwitch: (AllodiaPlan) -> Unit,
) {
    val ctx = LocalContext.current
    // Screen-local on purpose, unlike the purchase state next door: neither of these leaves the
    // app, so a dialog dismissed by rotating the phone is a dialog nobody has answered yet.
    var pending by remember { mutableStateOf<AllodiaPendingWrite?>(null) }
    val switchTo = allodiaSwitchTarget(subscription)

    if (switchTo != null) {
        TextButton(onClick = { pending = AllodiaPendingWrite.SWITCH }) {
            Text(
                when (switchTo) {
                    AllodiaPlan.YEARLY -> L10n.settings_subscription_switch_yearly(ctx)
                    AllodiaPlan.MONTHLY -> L10n.settings_subscription_switch_monthly(ctx)
                }
            )
        }
    }
    if (allodiaOffersRestart(subscription)) {
        TextButton(onClick = onResubscribe) {
            Text(L10n.settings_subscription_resubscribe(ctx))
        }
    }
    if (subscription.actions.canCancel) {
        TextButton(onClick = { pending = AllodiaPendingWrite.CANCEL }) {
            Text(L10n.settings_subscription_cancel(ctx))
        }
    }

    when (pending) {
        AllodiaPendingWrite.CANCEL ->
            ConfirmWrite(
                title = L10n.settings_subscription_cancel_title(ctx),
                // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are
                // not losing what they have already paid for. A cancellation with no date reads as
                // "it stops now", which is the one thing it does not do.
                body = L10n.settings_subscription_cancel_body(ctx, allodiaWriteDate(subscription)),
                confirm = L10n.settings_subscription_cancel(ctx),
                // **Never "Cancel" for the way out.** On this dialog that word is the thing being
                // asked about, so the button that does nothing has to say what it keeps.
                dismiss = L10n.settings_subscription_cancel_keep(ctx),
                destructive = true,
                onConfirm = {
                    pending = null
                    onCancel()
                },
                onDismiss = { pending = null },
            )
        AllodiaPendingWrite.SWITCH ->
            ConfirmWrite(
                title = L10n.settings_subscription_switch_title(ctx),
                // ⚠️ **No price here, deliberately.** What this subscriber is charged is not
                // today's list price, because a price change never reaches somebody who already
                // subscribed, and quoting the list price would tell a long-standing subscriber a
                // number they will not be charged. The amount comes back from the switch itself
                // and is said afterwards.
                body = L10n.settings_subscription_switch_body(ctx, allodiaWriteDate(subscription)),
                confirm = L10n.action_update(ctx),
                dismiss = L10n.action_cancel(ctx),
                destructive = false,
                onConfirm = {
                    pending = null
                    switchTo?.let(onSwitch)
                },
                onDismiss = { pending = null },
            )
        null -> Unit
    }
}

@Composable
private fun ConfirmWrite(
    title: String,
    body: String,
    confirm: String,
    dismiss: String,
    destructive: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(body) },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                colors =
                    if (destructive) {
                        ButtonDefaults.textButtonColors(
                            contentColor = MaterialTheme.colorScheme.error
                        )
                    } else {
                        ButtonDefaults.textButtonColors()
                    },
            ) {
                Text(confirm)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(dismiss) } },
    )
}

// The day both confirmations talk about: what has already been paid for.
//
// The raw string when it cannot be read, which is the same fallback the card's own dates take: a
// sentence with the wire in it is still true, and one with no date at all is not.
@Composable
private fun allodiaWriteDate(subscription: AllodiaSubscription): String {
    val locale: Locale = allodiaReaderLocale()
    val raw = subscription.own?.currentPeriodEnd ?: subscription.currentPeriodEnd ?: return ""
    return allodiaDate(raw, locale) ?: raw
}
