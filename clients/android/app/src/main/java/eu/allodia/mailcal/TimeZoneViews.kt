// The in-app display-timezone affordances for the Android client: the gear action
// that opens an IANA-zone selector, the selector dialog itself, and the prompt shown when
// the Rust core reports a device-zone change awaiting the user's choice. The active zone
// and any pending change are owned by the Rust core (TimeZoneSnapshot); this file only
// renders them and dispatches the matching intents. Kept in its own file so MainActivity
// stays under the 500-line limit (gradle auto-globs the package, so no build-file change).
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.TimeZoneSnapshot
import uniffi.mailcal_bindings.availableTimeZones

// The Settings-screen time-zone row: shows the active zone and opens the zone selector on tap.
// The active zone and any pending device-zone change live in the snapshot (null until the first
// settings pull); this only surfaces them. Selecting a zone dispatches Intent.SetTimeZone via
// [onSelect]. (The pending-change prompt is a separate overlay, see TimeZoneChangePrompt.)
@androidx.compose.runtime.Composable
internal fun TimeZoneSettingsRow(
    timeZone: TimeZoneSnapshot?,
    onSelect: (id: String) -> Unit,
) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val active = timeZone?.active.orEmpty()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { open = true }
            .padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = active.ifEmpty { "—" },
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        Icon(
            painter = painterResource(R.drawable.ic_arrow_drop_down),
            contentDescription = L10n.tz_picker_title(ctx),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
    if (open) {
        TimeZoneSelectorDialog(
            active = active,
            onSelect = { id ->
                onSelect(id)
                open = false
            },
            onDismiss = { open = false },
        )
    }
}

// A scrollable list of the engine's IANA zones (its bundled tzdb, sourced over the FFI so
// every client offers the same set the engine can localise against), with a small filter
// field since the full list is long. Tapping a row dispatches the selection and closes the
// dialog; the active zone is marked.
@androidx.compose.runtime.Composable
private fun TimeZoneSelectorDialog(
    active: String,
    onSelect: (id: String) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    var filter by remember { mutableStateOf("") }
    // The engine's authoritative zone list (already sorted, de-duplicated, and limited to
    // zones it can resolve), shared by every client instead of the host OS zone set.
    val zones = remember { availableTimeZones() }
    val shown = remember(filter, zones) {
        if (filter.isBlank()) zones else zones.filter { it.contains(filter, ignoreCase = true) }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(L10n.tz_picker_title(ctx)) },
        text = {
            Column {
                Text(
                    text = L10n.tz_picker_active(ctx, active.ifEmpty { "—" }),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedTextField(
                    value = filter,
                    onValueChange = { filter = it },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    placeholder = { Text(L10n.tz_filter_placeholder(ctx)) },
                )
                Spacer(modifier = Modifier.height(8.dp))
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(320.dp),
                ) {
                    items(shown, key = { it }) { zone ->
                        val selected = zone == active
                        Text(
                            text = zone,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (selected) {
                                MaterialTheme.colorScheme.primary
                            } else {
                                MaterialTheme.colorScheme.onSurface
                            },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelect(zone) }
                                .padding(vertical = 10.dp),
                        )
                    }
                }
            }
        },
        // Selection happens by tapping a row; no separate confirm affordance.
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
        },
    )
}

// Shown only when the core has a pending device-zone change (snapshot.pendingDevice != null):
// the device moved to a new zone and the user must adopt it or keep the current one. Confirm
// dispatches Intent.AcceptTimeZoneChange, dismiss dispatches Intent.DismissTimeZoneChange.
@androidx.compose.runtime.Composable
internal fun TimeZoneChangePrompt(
    timeZone: TimeZoneSnapshot?,
    onAccept: () -> Unit,
    onDismiss: () -> Unit,
) {
    val pending = timeZone?.pendingDevice ?: return
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(L10n.tz_changed_title(ctx)) },
        text = { Text(L10n.tz_changed_message(ctx, pending)) },
        confirmButton = {
            TextButton(onClick = onAccept) { Text(L10n.action_update(ctx)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(L10n.tz_keep(ctx, timeZone.active)) }
        },
    )
}
