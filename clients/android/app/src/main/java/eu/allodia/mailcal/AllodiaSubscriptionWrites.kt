// The three things somebody may do to the subscription Allodia bills directly: stop the recurring
// charge, start it again, and move between the two periods.
//
// ⚠️ **Only Allodia's own subscription, never a store's.** A store's is changed at that store, so
// the section offers its page for those and none of this. Which buttons exist, whether one runs and
// whether the core takes the press are all the card's; each sends an `AllodiaIntent`.
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.AllodiaPlan
import uniffi.mailcal_bindings.AllodiaSubscriptionView

// Which confirmation is on screen, if any.
//
// Both of these change what somebody is charged, and neither says so anywhere else: a cancellation
// is silent until the period runs out, and a switch is silent until a date that may be a month
// away. So both are asked before they are done.
private enum class AllodiaPendingWrite {
    CANCEL,
    SWITCH,
}

@Composable
internal fun AllodiaSubscriptionWrites(view: AllodiaSubscriptionView, onWrite: (AllodiaIntent) -> Unit) {
    val ctx = LocalContext.current
    // Screen-local on purpose: a dialog dismissed by rotating the phone is one nobody has answered.
    var pending by remember { mutableStateOf<AllodiaPendingWrite?>(null) }
    val switch = view.switch

    switch?.let {
        ControlTextButton(
            label = when (it.to) {
                AllodiaPlan.YEARLY -> L10n.settings_subscription_switch_yearly(ctx)
                AllodiaPlan.MONTHLY -> L10n.settings_subscription_switch_monthly(ctx)
            },
            control = it.control,
            tag = "subscription-switch",
        ) { pending = AllodiaPendingWrite.SWITCH }
    }
    view.resubscribe?.let { control ->
        ControlTextButton(
            label = L10n.settings_subscription_resubscribe(ctx),
            control = control,
            tag = "subscription-resubscribe",
        ) { onWrite(AllodiaIntent.Resubscribe) }
    }
    view.cancel?.let { control ->
        ControlTextButton(
            label = L10n.settings_subscription_cancel(ctx),
            control = control,
            tag = "subscription-cancel",
        ) { pending = AllodiaPendingWrite.CANCEL }
    }

    val paidThrough = allodiaPaidThrough(view)
    when (pending) {
        AllodiaPendingWrite.CANCEL ->
            ConfirmWrite(
                title = L10n.settings_subscription_cancel_title(ctx),
                // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are
                // not losing what they have already paid for.
                body = L10n.settings_subscription_cancel_body(ctx, paidThrough),
                confirm = L10n.settings_subscription_cancel(ctx),
                // **Never "Cancel" for the way out.** On this dialog that word is the thing being
                // asked about, so the button that does nothing has to say what it keeps.
                dismiss = L10n.settings_subscription_cancel_keep(ctx),
                destructive = true,
                onConfirm = {
                    pending = null
                    onWrite(AllodiaIntent.CancelSubscription)
                },
                onDismiss = { pending = null },
            )
        AllodiaPendingWrite.SWITCH ->
            ConfirmWrite(
                title = L10n.settings_subscription_switch_title(ctx),
                // ⚠️ **No price here, deliberately.** A price change never reaches somebody who
                // already subscribed, so today's list price is not what they will be charged. The
                // amount comes back from the switch itself and is said afterwards.
                body = L10n.settings_subscription_switch_body(ctx, paidThrough),
                confirm = L10n.action_update(ctx),
                dismiss = L10n.action_cancel(ctx),
                destructive = false,
                onConfirm = {
                    pending = null
                    switch?.let { onWrite(AllodiaIntent.SwitchInterval(it.to)) }
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
                        ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error)
                    } else {
                        ButtonDefaults.textButtonColors()
                    },
                modifier = Modifier.testTag("subscription-confirm"),
            ) {
                Text(confirm)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(dismiss) } },
    )
}

// The day both confirmations talk about: what has already been paid for. The wire when this build
// cannot read it: a sentence with the wire in it is still true, and one with no date at all is not.
@Composable
private fun allodiaPaidThrough(view: AllodiaSubscriptionView): String {
    val raw = view.paidThrough ?: return ""
    return allodiaDate(raw, allodiaReaderLocale()) ?: raw
}
