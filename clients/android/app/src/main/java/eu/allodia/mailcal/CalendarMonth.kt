// The month grid: six weeks of day cells, each listing what happens that day.
//
// A different layout from the time grid, not the same one with more columns, a cell has no hour
// axis and no overlap solving, only a list. The core hands back every event on every day; how many
// chips fit is a question of how tall a cell is on *this* screen, so the cap and the "+N more" are
// computed here (the same division of labour as the all-day banner).
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import java.util.Locale
import uniffi.mailcal_bindings.MonthCell
import uniffi.mailcal_bindings.MonthPage

private val CHIP_HEIGHT = 15.dp
private val CHIP_CORNER = 2.dp
private const val MONTH_WEEKS = 6

/** How many chips fit in the space a cell leaves below its date number. */
internal fun monthChipCapacity(chipArea: Dp): Int =
    ((chipArea + 2.dp) / CHIP_HEIGHT).toInt().coerceAtLeast(0)

/**
 * How many chips a cell actually draws, and how many it then has to admit to hiding.
 *
 * The subtlety: if a cell has exactly one more event than fits, drawing "+1 more" in the last slot
 * *costs* a slot, so it would hide two to report one. In that case draw the event instead. The
 * overflow row only earns its place when it stands for more than it displaces.
 */
internal fun monthChipsShown(total: Int, capacity: Int): Int =
    if (total <= capacity) total else (capacity - 1).coerceAtLeast(0)

/** The month, drawn. */
@Composable
internal fun CalendarMonthGrid(
    page: MonthPage,
    today: LocalDate,
    locale: Locale,
    weekStartsMonday: Boolean,
    onOpenEvent: (EventOpen) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dark = LocalAppDark.current
    if (page.cells.size < MONTH_WEEKS * 7) return

    Column(modifier = modifier.fillMaxSize()) {
        MonthWeekdayHeader(locale = locale, weekStartsMonday = weekStartsMonday)
        HorizontalDivider()
        if (!page.isMaterialized) {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        // Each week is an equal share of what's left, so the grid fills the screen exactly and never
        // scrolls, a month you have to scroll is a month you cannot see.
        Column(modifier = Modifier.fillMaxSize()) {
            repeat(MONTH_WEEKS) { week ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                ) {
                    for (column in 0 until 7) {
                        val cell = page.cells[week * 7 + column]
                        MonthDayCell(
                            cell = cell,
                            page = page,
                            isToday = cell.date == today.toString(),
                            dark = dark,
                            onOpenEvent = onOpenEvent,
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxSize(),
                        )
                    }
                }
                HorizontalDivider()
            }
        }
    }
}

// The weekday headings, in the locale's own abbreviations and starting on the user's chosen day.
@Composable
private fun MonthWeekdayHeader(locale: Locale, weekStartsMonday: Boolean) {
    // A reference week: 2026-07-06 is a Monday, 2026-07-05 a Sunday.
    val first = if (weekStartsMonday) LocalDate.of(2026, 7, 6) else LocalDate.of(2026, 7, 5)
    Row(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        for (offset in 0 until 7) {
            Text(
                text = weekdayShort(first.plusDays(offset.toLong()), locale),
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                maxLines = 1,
            )
        }
    }
}

// One day: its date number, then what happens on it, then "+N more" if the rest won't fit.
@Composable
private fun MonthDayCell(
    cell: MonthCell,
    page: MonthPage,
    isToday: Boolean,
    dark: Boolean,
    onOpenEvent: (EventOpen) -> Unit,
    modifier: Modifier = Modifier,
) {
    val date = parseIsoDate(cell.date)
    Box(modifier = modifier.padding(horizontal = 1.dp)) {
        Column(modifier = Modifier.fillMaxSize()) {
            // The date number. Days of the neighbouring months are dimmed, without that, the 1st of
            // next month reads as part of this one and you tap into the wrong month.
            Box(
                modifier = Modifier
                    .padding(top = 2.dp)
                    .size(18.dp)
                    .align(Alignment.CenterHorizontally)
                    .then(
                        if (isToday) {
                            Modifier
                                .clip(CircleShape)
                                .background(MaterialTheme.colorScheme.primary)
                        } else {
                            Modifier
                        },
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "${date.dayOfMonth}",
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = if (isToday) FontWeight.Bold else FontWeight.Normal,
                    color = when {
                        isToday -> MaterialTheme.colorScheme.onPrimary
                        cell.inMonth -> MaterialTheme.colorScheme.onSurface
                        else -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
                    },
                )
            }
            MonthChips(cell = cell, page = page, dark = dark, onOpenEvent = onOpenEvent)
        }
    }
}

@Composable
private fun ColumnScope.MonthChips(
    cell: MonthCell,
    page: MonthPage,
    dark: Boolean,
    onOpenEvent: (EventOpen) -> Unit,
) {
    val ctx = LocalContext.current
    BoxWithConstraints(
        modifier = Modifier
            .weight(1f)
            .fillMaxWidth(),
    ) {
        // How many chips fit is decided here, from the cell's real height, the core does not guess
        // at a phone's row height, so it hands back every event and lets this cap them.
        val capacity = monthChipCapacity(maxHeight)
        val total = cell.chips.size
        val shown = monthChipsShown(total, capacity)
        val hidden = total - shown

        Column(modifier = Modifier.fillMaxWidth()) {
            cell.chips.take(shown).forEach { chip ->
                val calendar = page.calendars.rowFor(chip.account, chip.calendar)
                val swatch = calendar.swatchOrFallback(dark)
                // An invitation nobody has answered is a hold, not a commitment: faded, dashed and
                // hatched (CalendarParticipation.kt), and its spoken label says so, the drawing
                // alone is invisible to a screen reader.
                val awaiting = isAwaitingResponse(chip.participation)
                val title = chip.title.ifEmpty { L10n.event_no_title(ctx) }
                Text(
                    text = title,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 1.dp)
                        .clip(RoundedCornerShape(CHIP_CORNER))
                        .background(parseHexColor(swatch.background).holdFill(awaiting))
                        .holdChip(awaiting, parseHexColor(swatch.border), CHIP_CORNER)
                        .clickable { onOpenEvent(EventOpen(chip.account, chip.event, chip.occurrenceStart)) }
                        .padding(horizontal = 2.dp)
                        .semantics {
                            contentDescription = if (awaiting) {
                                "$title, ${L10n.a11y_invitation_awaiting_response(ctx)}"
                            } else {
                                title
                            }
                        },
                    style = MaterialTheme.typography.labelSmall,
                    fontSize = 9.sp,
                    lineHeight = 11.sp,
                    color = parseHexColor(swatch.text),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (hidden > 0) {
                Text(
                    text = L10n.calendar_all_day_more(ctx, hidden),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 2.dp),
                    style = MaterialTheme.typography.labelSmall,
                    fontSize = 9.sp,
                    lineHeight = 11.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
        }
    }
}
