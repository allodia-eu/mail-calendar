// The found card's section header and its calendar section. Split from AccountSetupDetect.kt,
// which had reached the size limit; nothing here holds the card's state.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.platform.LocalContext

// A small section header, e.g. "✉  Email" / "📅  Calendar", grouping the found card.
@Composable
internal fun SectionHeader(icon: String, label: String) {
    Text("$icon $label", style = MaterialTheme.typography.titleSmall)
}

// The Calendar section of the found card. When detection discovered a CalDAV endpoint the
// toggle is pre-checked (opt-out) and its host is shown; otherwise it's an opt-in toggle that
// reveals a manual CalDAV field. Calendar reuses the IMAP credentials at connect.
@Composable
internal fun CalendarSection(
    discovered: String?,
    enabled: Boolean,
    onEnabledChange: (Boolean) -> Unit,
    url: String,
    onUrlChange: (String) -> Unit,
) {
    val ctx = LocalContext.current
    SectionHeader("📅", L10n.setup_detect_section_calendar(ctx))
    val label = if (discovered != null) L10n.setup_detect_calendar_enable(ctx) else L10n.setup_detect_calendar_add(ctx)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = enabled, onCheckedChange = onEnabledChange)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
    if (enabled) {
        if (discovered != null) {
            Text(urlHost(discovered), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            SetupField(url, onUrlChange, L10n.setup_field_caldav(ctx), L10n.setup_hint_caldav(ctx))
        }
    }
}

// The host of a discovered URL (CalDAV endpoint, JMAP base), for a compact confirmation line:
// so an untrusted result's "check the server names" has a name to check; the full URL is the
// fallback if it somehow doesn't parse.
internal fun urlHost(url: String): String = runCatching { java.net.URI(url).host }.getOrNull() ?: url
