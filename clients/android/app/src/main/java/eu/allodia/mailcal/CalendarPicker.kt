// The calendar picker, a bottom sheet that chooses which calendar a new event lands in, grouped by
// account (a calendar id is only unique within its account) and filtered to the ones we can write to.
// This is the app's first ModalBottomSheet, matching the "Agenda selecteren" sheet of the platform
// calendar the user knows.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.CalendarRow

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CalendarPickerSheet(
    calendars: List<CalendarRow>,
    selected: CalendarChoice?,
    onPick: (CalendarChoice) -> Unit,
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val dark = LocalAppDark.current
    // Only writable calendars can take a new event; group them by account for the section headers.
    val byAccount = calendars.filter { it.canWrite }.groupBy { it.account }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(
            text = L10n.event_pick_calendar(ctx),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 8.dp),
        )
        LazyColumn(modifier = Modifier.navigationBarsPadding()) {
            byAccount.forEach { (account, rows) ->
                item(key = "acct-$account") {
                    Text(
                        text = account,
                        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 12.dp, bottom = 4.dp),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                items(rows, key = { "${it.account}:${it.id}" }) { calendar ->
                    val chosen = selected?.account == calendar.account && selected.id == calendar.id
                    val swatch = calendar.color.swatch(dark)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onPick(CalendarChoice(calendar.account, calendar.id, calendar.name)) }
                            .padding(horizontal = 24.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        androidx.compose.foundation.layout.Box(
                            modifier = Modifier
                                .size(18.dp)
                                .clip(CircleShape)
                                .background(parseHexColor(swatch.background))
                                .border(1.dp, parseHexColor(swatch.border), CircleShape),
                        )
                        Spacer(Modifier.width(16.dp))
                        Text(
                            text = calendar.name,
                            modifier = Modifier.weight(1f),
                            style = MaterialTheme.typography.bodyLarge,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (chosen) {
                            Icon(
                                painter = painterResource(R.drawable.ic_check),
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                            )
                        }
                    }
                }
            }
        }
    }
}
