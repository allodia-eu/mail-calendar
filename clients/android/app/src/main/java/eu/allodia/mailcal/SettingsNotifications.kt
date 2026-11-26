// Settings → Notifications: the new-mail notification switch, as the core decides it
// (`notifications_page`), and, while the battery exemption is missing, the "Background mail
// delivery" card that explains and requests it (docs/background-sync.md). Split out of
// SettingsScreen.kt so each file stays under the 500-line limit (gradle auto-globs the package).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import uniffi.mailcal_bindings.NotificationsPage

internal const val NEW_MAIL_SWITCH = "settings-notifications-new-mail"

// The whole Notifications category: the switch's group card, then (only while missing) the
// battery-exemption card.
@Composable
internal fun NotificationsSettings(page: NotificationsPage, onSetNewMail: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    SettingsGroupCard(
        L10n.settings_notifications_heading(ctx),
        L10n.settings_notifications_description(ctx),
    ) {
        NewMailSwitch(page.newMail, onSetNewMail)
    }
    BackgroundDeliveryCard()
}

// The new-mail switch draws the core's value and sends the change; the SETTINGS signal it raises
// brings the page back. Off, a pass still runs and moves the core's marks, so turning it on again
// announces only what arrives from then on.
@Composable
private fun NewMailSwitch(enabled: Boolean, onSetNewMail: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            if (enabled) L10n.settings_toggle_on(ctx) else L10n.settings_toggle_off(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Switch(
            checked = enabled,
            onCheckedChange = onSetNewMail,
            modifier = Modifier.testTag(NEW_MAIL_SWITCH),
        )
    }
}

// Whether Android is letting the background sync keep its schedule, and the one-tap way to fix it
// (docs/background-sync.md). Shown only when the exemption is *missing*, once granted, this is
// nothing the user needs to think about again, so it disappears rather than becoming a permanent
// row of clutter. The state is re-read on resume, because the user grants it in a system dialog
// that we are not told the outcome of.
@Composable
private fun BackgroundDeliveryCard() {
    val ctx = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    var exempt by remember { mutableStateOf(BatteryOptimization.isExempt(ctx)) }

    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val now = BatteryOptimization.isExempt(ctx)
                // Log the *transition*, not the state: the system dialog never tells us its outcome,
                // so coming back from it is the only moment we learn what the user chose. Every
                // background wake already records the standing state, this is what dates the change,
                // which is what a "it used to be fine and now it's slow" report turns on.
                if (now != exempt) {
                    FileLog.append(
                        "INFO",
                        "android-ui",
                        if (now) {
                            "battery: user allowed unrestricted background use; the sync can now keep its period"
                        } else {
                            "battery: unrestricted background use revoked; the OS may now defer a pass by hours"
                        },
                    )
                }
                exempt = now
            }
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }

    if (exempt) {
        return
    }
    Spacer(modifier = Modifier.height(8.dp))
    SettingsGroupCard(
        L10n.settings_battery_heading(ctx),
        L10n.settings_battery_description(ctx),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
        ) {
            TextButton(onClick = { ctx.startActivity(BatteryOptimization.requestIntent(ctx)) }) {
                Text(L10n.settings_battery_allow(ctx))
            }
        }
    }
}
