// Which sent mail to learn from (docs/ai.md rule 11, `ChooseRange`): the core's presets, then a
// range of the person's own between two days the core bounds. Days arrive and leave as `YYYY-MM-DD`
// whole days in UTC, the date picker's own reckoning, so no day moves between the two.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import uniffi.mailcal_bindings.StyleLearnRange
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningStage

internal const val LEARN_SINCE_TAG = "learn-range-since"
internal const val LEARN_UNTIL_TAG = "learn-range-until"

/** A `YYYY-MM-DD` day as the date picker counts it: midnight UTC, in milliseconds. */
internal fun dayMillis(day: String): Long? =
    runCatching { LocalDate.parse(day).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() }.getOrNull()

/** The day the date picker chose, as the core reads one. */
internal fun millisDay(millis: Long): String = Instant.ofEpochMilli(millis).atOffset(ZoneOffset.UTC).toLocalDate().toString()

@Composable
internal fun LearnRangeChoice(stage: StyleLearningStage.ChooseRange, send: (StyleLearningIntent) -> Unit) {
    val ctx = LocalContext.current
    val locale = catalogLocale(LocalContext.current)
    val selected = stage.selected
    Column {
        // The last preset is a range of the person's own, carrying the days it starts from.
        stage.presets.forEach { range ->
            ChoiceRow(
                label = learnRangeText(ctx, range, locale),
                selected = if (range is StyleLearnRange.Between) selected is StyleLearnRange.Between else selected == range,
                tag = learnRangeTag(range),
                note = if (range == StyleLearnRange.UntilEndOfLastYear) L10n.learn_range_until_hint(ctx) else null,
            ) { if (!(range is StyleLearnRange.Between && selected is StyleLearnRange.Between)) send(StyleLearningIntent.SelectRange(range)) }
        }
        val between = selected as? StyleLearnRange.Between
        if (between != null) {
            Column(modifier = Modifier.padding(start = 52.dp)) {
                DayButton(L10n.learn_range_from(ctx), between.since, stage, LEARN_SINCE_TAG) {
                    send(StyleLearningIntent.SelectRange(between.copy(since = it)))
                }
                DayButton(L10n.learn_range_until_label(ctx), between.until, stage, LEARN_UNTIL_TAG) {
                    send(StyleLearningIntent.SelectRange(between.copy(until = it)))
                }
            }
        }
    }
}

// A day's label and a button showing it, which opens the date picker bounded by the core's days.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DayButton(label: String, day: String, stage: StyleLearningStage.ChooseRange, tag: String, onPick: (String) -> Unit) {
    val ctx = LocalContext.current
    val locale = catalogLocale(LocalContext.current)
    var picking by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 4.dp)) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        OutlinedButton(onClick = { picking = true }, modifier = Modifier.testTag(tag)) { Text(dayText(day, locale)) }
    }
    if (!picking) return
    val first = stage.earliest?.let(::dayMillis)
    val last = dayMillis(stage.latest)
    val state = rememberDatePickerState(
        initialSelectedDateMillis = dayMillis(day),
        selectableDates = object : SelectableDates {
            override fun isSelectableDate(utcTimeMillis: Long): Boolean =
                (first == null || utcTimeMillis >= first) && (last == null || utcTimeMillis <= last)
        },
    )
    DatePickerDialog(
        onDismissRequest = { picking = false },
        confirmButton = {
            TextButton(onClick = {
                picking = false
                state.selectedDateMillis?.let { onPick(millisDay(it)) }
            }) { Text(L10n.action_done(ctx)) }
        },
        dismissButton = { TextButton(onClick = { picking = false }) { Text(L10n.action_cancel(ctx)) } },
    ) { DatePicker(state = state) }
}
