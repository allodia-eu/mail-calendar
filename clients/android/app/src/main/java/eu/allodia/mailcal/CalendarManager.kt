// The calendar manager: which calendars are drawn, and in what colour.
//
// Grouped by account, because a calendar id is only unique within its account, two accounts can
// each have a "Work", and showing them in one flat list would leave the user unable to tell which is
// which. Every toggle and colour is persisted by the core and applied at page-read time, so the grid
// redraws immediately with no sync and no network.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import uniffi.mailcal_bindings.CalendarRow

/** The manager screen: every account's calendars, each with a colour dot and a visibility tick. */
@Composable
internal fun CalendarManagerScreen(
    calendars: List<CalendarRow>,
    palette: List<String>,
    onSetVisible: (account: String, calendar: String, visible: Boolean) -> Unit,
    onSetColor: (account: String, calendar: String, hex: String?) -> Unit,
    onBack: () -> Unit,
) {
    val ctx = LocalContext.current
    // The calendar whose colour picker is open, or null.
    var picking by remember { mutableStateOf<CalendarRow?>(null) }
    // Group by account, keeping each account's calendars in the order the core listed them (which is
    // the order their fallback colours were assigned from).
    val byAccount = remember(calendars) { calendars.groupBy { it.account } }

    // An OPAQUE surface: the manager is composed over the calendar, and without a background of its
    // own the grid shows straight through it, both drawn, both unreadable.
    Surface(modifier = Modifier.fillMaxSize()) {
    Column(modifier = Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(L10n.calendar_manage(ctx), style = MaterialTheme.typography.titleLarge)
            TextButton(onClick = onBack) { Text(L10n.action_done(ctx)) }
        }
        HorizontalDivider()

        if (calendars.isEmpty()) {
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = L10n.calendar_manage_empty(ctx),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            LazyColumn(modifier = Modifier.weight(1f)) {
                byAccount.forEach { (account, rows) ->
                    item(key = "acct-$account") {
                        Text(
                            text = account,
                            modifier = Modifier.padding(
                                start = 16.dp,
                                end = 16.dp,
                                top = 16.dp,
                                bottom = 4.dp,
                            ),
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }
                    items(rows, key = { "${it.account}:${it.id}" }) { calendar ->
                        CalendarManagerRow(
                            calendar = calendar,
                            onToggle = { visible ->
                                onSetVisible(calendar.account, calendar.id, visible)
                            },
                            onPickColor = { picking = calendar },
                        )
                    }
                }
            }
        }
    }
    }

    picking?.let { calendar ->
        ColorPickerDialog(
            calendar = calendar,
            palette = palette,
            onPick = { hex ->
                onSetColor(calendar.account, calendar.id, hex)
                picking = null
            },
            onDismiss = { picking = null },
        )
    }
}

// One calendar: its colour (tap to change), its name, and whether it's drawn.
@Composable
private fun CalendarManagerRow(
    calendar: CalendarRow,
    onToggle: (Boolean) -> Unit,
    onPickColor: () -> Unit,
) {
    val ctx = LocalContext.current
    val dark = LocalAppDark.current
    val swatch = calendar.color.swatch(dark)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onToggle(!calendar.visible) }
            .padding(horizontal = 16.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(28.dp)
                .clickable(onClick = onPickColor)
                .semantics {
                    contentDescription = L10n.calendar_pick_color(ctx, calendar.name)
                },
            contentAlignment = Alignment.Center,
        ) {
            Box(
                modifier = Modifier
                    .size(20.dp)
                    .clip(CircleShape)
                    .background(parseHexColor(swatch.background))
                    .border(1.dp, parseHexColor(swatch.border), CircleShape),
            )
        }
        Spacer(modifier = Modifier.width(12.dp))
        Text(
            text = calendar.name,
            modifier = Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Checkbox(checked = calendar.visible, onCheckedChange = onToggle)
    }
}

// The palette, as swatches. The colours come from the core, a client cannot invent one, and Allodia
// Orange is deliberately absent from the list because it means "action" in this product.
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ColorPickerDialog(
    calendar: CalendarRow,
    palette: List<String>,
    onPick: (String?) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(calendar.name) },
        text = {
            Column {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    palette.forEach { hex ->
                        val selected = hex.equals(calendar.color.hex, ignoreCase = true)
                        Box(
                            modifier = Modifier
                                .size(40.dp)
                                .clip(CircleShape)
                                .background(parseHexColor(hex))
                                .then(
                                    if (selected) {
                                        Modifier.border(
                                            3.dp,
                                            MaterialTheme.colorScheme.onSurface,
                                            CircleShape,
                                        )
                                    } else {
                                        Modifier
                                    },
                                )
                                .clickable { onPick(hex) }
                                .semantics { contentDescription = hex },
                        )
                    }
                }
                Spacer(modifier = Modifier.height(12.dp))
                // Clearing the override hands the calendar back to whatever colour its server sent.
                TextButton(onClick = { onPick(null) }) {
                    Text(L10n.calendar_color_reset(ctx))
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
        },
    )
}
