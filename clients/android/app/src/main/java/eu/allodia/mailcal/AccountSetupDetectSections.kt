// The found card's section header, and the host it names a server by. Split from AccountSetupDetect.kt,
// which had reached the size limit; nothing here holds the card's state.
package eu.allodia.mailcal

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable

// A small section header, e.g. "✉  Email" / "📅  Calendar", grouping the found card.
@Composable
internal fun SectionHeader(icon: String, label: String) {
    Text("$icon $label", style = MaterialTheme.typography.titleSmall)
}

// The host of a discovered URL (CalDAV endpoint, JMAP base), for a compact confirmation line:
// so an untrusted result's "check the server names" has a name to check; the full URL is the
// fallback if it somehow doesn't parse.
internal fun urlHost(url: String): String = runCatching { java.net.URI(url).host }.getOrNull() ?: url
