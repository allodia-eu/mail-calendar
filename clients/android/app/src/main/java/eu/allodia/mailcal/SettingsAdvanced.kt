// Settings → Advanced as the core decides it (`advanced_page`): the reset of the local database,
// running from the confirmation until the mail is downloaded again, and what the last one came to.
// A phone sets no endpoint for AI assistants, so the page carries no assistant panel here.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AdvancedPage
import uniffi.mailcal_bindings.ResetOutcome

internal const val RESET_BUTTON = "settings-advanced-reset"
internal const val RESET_OUTCOME = "settings-advanced-reset-outcome"

/** The reset card. [onRequestReset] asks the person first; the reset itself starts on their yes. */
@Composable
internal fun AdvancedSettings(page: AdvancedPage, onRequestReset: () -> Unit) {
    val ctx = LocalContext.current
    SettingsGroupCard(L10n.action_reset_database(ctx), L10n.settings_advanced_reset_description(ctx)) {
        ControlButton(
            label = L10n.action_reset_database(ctx),
            control = page.reset,
            tag = RESET_BUTTON,
            destructive = true,
            onClick = onRequestReset,
        )
        page.resetOutcome?.let { outcome ->
            Text(
                when (outcome) {
                    ResetOutcome.DONE -> L10n.settings_advanced_reset_done(ctx)
                    ResetOutcome.FAILED -> L10n.settings_advanced_reset_failed(ctx)
                },
                style = MaterialTheme.typography.bodySmall,
                color = when (outcome) {
                    ResetOutcome.DONE -> MaterialTheme.colorScheme.onSurfaceVariant
                    ResetOutcome.FAILED -> MaterialTheme.colorScheme.error
                },
                modifier = Modifier
                    .padding(top = 8.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite }
                    .testTag(RESET_OUTCOME),
            )
        }
    }
}
