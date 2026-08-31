// The two controls the usage-statistics consent is made of, plus the Settings card that hosts the
// withdrawal. Shared so the welcome screen and Settings → Privacy cannot drift apart: they ask the
// same question, show the same payload, and write through the same setter.
//
// GDPR Art. 7(3): withdrawing consent must be as easy as giving it. That is why the Settings switch
// is one tap in the same shape as the welcome screen's, not a buried confirmation flow.
package eu.allodia.mailcal

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp

/**
 * The switch and its label, the whole row being the target.
 *
 * `toggleable` rather than `clickable(role = Role.Switch)`: only `toggleable` publishes a
 * `ToggleableState`, so a screen reader can say *whether the switch is on*. With `clickable` it
 * announces "switch" and then nothing, which for a consent control is precisely the state the user
 * needs to hear. The `Switch` itself takes `onCheckedChange = null` so the row owns the semantics
 * and the control is not separately focusable.
 */
@Composable
internal fun AnalyticsSwitchRow(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = checked, role = Role.Switch, onValueChange = onCheckedChange),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.titleMedium)
        Switch(checked = checked, onCheckedChange = null)
    }
}

/**
 * "See exactly what we send", and behind it the literal bytes.
 *
 * The JSON comes from the core's own payload type, the same one the sink serializes, so this is
 * the payload, not a description of it. It is pulled lazily: an unopened panel costs nothing.
 *
 * Monospaced and scrolled sideways rather than wrapped. A re-flowed payload is a paraphrase, and
 * the entire point of this panel is that it is not one.
 */
@Composable
internal fun AnalyticsPayloadPanel(payloadPreview: () -> String) {
    val ctx = LocalContext.current
    var showing by remember { mutableStateOf(false) }

    TextButton(onClick = { showing = !showing }) {
        Text(L10n.welcome_analytics_preview(ctx))
    }
    if (showing) {
        Surface(
            color = MaterialTheme.colorScheme.surfaceVariant,
            shape = MaterialTheme.shapes.small,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                payloadPreview(),
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
                modifier = Modifier
                    .horizontalScroll(rememberScrollState())
                    .padding(12.dp),
            )
        }
    }
}

/** Settings → Privacy: the live switch, and the same payload panel the welcome screen showed. */
@Composable
internal fun AnalyticsCard(
    enabled: Boolean,
    onSetEnabled: (Boolean) -> Unit,
    payloadPreview: () -> String,
) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        AnalyticsSwitchRow(
            label = L10n.settings_analytics_toggle(ctx),
            checked = enabled,
            onCheckedChange = onSetEnabled,
        )
        AnalyticsPayloadPanel(payloadPreview)
    }
}
