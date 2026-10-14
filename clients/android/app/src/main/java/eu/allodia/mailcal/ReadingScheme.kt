// Which page a message is drawn on, and the two controls that change it: the sun/moon button on
// the reading screen's action row and the switch in Settings → Reading (docs/reading-dark-mode.md).
// Both set the one preference the core persists, so every message follows.
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.MessageScheme

// Dark exactly when the app is painted dark and the reader has left dark messages on. Known before
// the body arrives, so the canvas behind it is right from the first frame.
internal fun messageScheme(appDark: Boolean, darkMessages: Boolean): MessageScheme =
    if (appDark && darkMessages) MessageScheme.DARK else MessageScheme.LIGHT

// The sun/moon button. Its glyph and its name say what pressing it does: a sun on a dark message,
// a moon on a light one. The row draws it only while the app is dark.
@Composable
internal fun MessageSchemeToggle(scheme: MessageScheme, onSetDarkMessages: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    val dark = scheme == MessageScheme.DARK
    IconButton(onClick = { onSetDarkMessages(!dark) }) {
        Icon(
            painter = painterResource(if (dark) R.drawable.ic_light_mode else R.drawable.ic_dark_mode),
            contentDescription = if (dark) {
                L10n.reading_show_light_background(ctx)
            } else {
                L10n.reading_show_dark_background(ctx)
            },
        )
    }
}

// The same preference in Settings → Reading.
@Composable
internal fun DarkMessagesSwitch(checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(role = Role.Switch) { onCheckedChange(!checked) }
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            L10n.settings_dark_messages_toggle(ctx),
            modifier = Modifier.weight(1f).padding(end = 12.dp),
            style = MaterialTheme.typography.bodyLarge,
        )
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
