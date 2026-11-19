// The pictures a style is shown back in (docs/ai.md rule 15): the counts of what was read, the
// miniature letter, the greeting and sign-off bars, the tone cards and the phrase chips. Each is
// drawn from the core's record as it stands; `shown` false holds a picture hidden for its first
// frame so it plays in, and the detail passes it true so every value is drawn final.
package eu.allodia.mailcal

import androidx.compose.animation.core.animateIntAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import uniffi.mailcal_bindings.WritingStyleFacts
import uniffi.mailcal_bindings.WritingStyleHabit
import uniffi.mailcal_bindings.WritingStyleHabits
import uniffi.mailcal_bindings.WritingStyleLetter
import uniffi.mailcal_bindings.WritingStyleSpan

private val NUMERAL = TextStyle(fontWeight = FontWeight.SemiBold, fontFeatureSettings = "tnum")

/** What a style was learned from: the address, then how many messages since when, in which languages. */
@Composable
internal fun StyleFactsView(facts: WritingStyleFacts, zone: ZoneId, shown: Boolean) {
    val ctx = LocalContext.current
    val locale = catalogLocale(LocalContext.current)
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        if (facts.learnedFrom.isNotEmpty()) {
            Text(
                L10n.reveal_based_on(ctx, address = facts.learnedFrom),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val since = facts.since?.let { epochDayText(it, zone, locale) }
        val spoken = since?.let {
            L10n.reveal_stats_a11y(ctx, facts.messages.toInt(), it, languagesText(facts.languages, locale))
        }
        Row(
            modifier = Modifier
                .height(IntrinsicSize.Min)
                .then(if (spoken != null) Modifier.clearAndSetSemantics { contentDescription = spoken } else Modifier),
        ) {
            Stat(L10n.reveal_stat_messages(ctx), facts.messages.toInt(), shown, 0)
            StatDivider()
            Stat(L10n.reveal_stat_languages(ctx), facts.languages.size, shown, 120)
        }
        if (since != null) {
            Column(modifier = Modifier.clearAndSetSemantics {}.entrance(shown, Entrance.RISE, 240)) {
                Text(L10n.reveal_stat_since(ctx), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(since, style = MaterialTheme.typography.titleLarge.merge(NUMERAL))
            }
        }
        LanguageChips(facts.languages, shown)
    }
}

@Composable
private fun StatDivider() {
    Box(
        modifier = Modifier
            .padding(horizontal = 20.dp)
            .width(1.dp)
            .fillMaxHeight()
            .background(MaterialTheme.colorScheme.outlineVariant),
    )
}

// A count from nought, in a box as wide as the final value so nothing beside it moves.
@Composable
private fun Stat(label: String, target: Int, shown: Boolean, delayMillis: Int) {
    val value by animateIntAsState(if (shown) target else 0, tween(900, delayMillis, COUNT), label = "count")
    val style = MaterialTheme.typography.displayMedium.merge(NUMERAL)
    Column(modifier = Modifier.entrance(shown, Entrance.RISE, delayMillis)) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Box {
            Text(target.toString(), style = style, modifier = Modifier.alpha(0f))
            Text(value.toString(), style = style)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LanguageChips(codes: List<String>, shown: Boolean) {
    val locale = catalogLocale(LocalContext.current)
    val colours = with(MaterialTheme.colorScheme) { listOf(primary, tertiary, secondary, inversePrimary, outline) }
    // The counts above already name the languages.
    FlowRow(
        modifier = Modifier.clearAndSetSemantics {},
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        codes.forEachIndexed { index, code ->
            Row(
                modifier = Modifier
                    .entrance(shown, Entrance.POP, 600 + 80 * index)
                    .heightIn(min = 32.dp)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh, CircleShape)
                    .padding(start = 12.dp, end = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(modifier = Modifier.size(8.dp).background(colours[index % colours.size], CircleShape))
                Text(languageText(code, locale), style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}

/** A typical reply: the miniature letter, then its length and shape. */
@Composable
internal fun StyleLetterView(letter: WritingStyleLetter, shown: Boolean) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Letter(letter, shown)
        Text(
            L10n.reveal_letter_caption(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (letter.words > 0u) {
            val count = letter.words.toInt()
            val sentence = L10n.reveal_length(ctx, count)
            val number = count.toString()
            val at = sentence.indexOf(number)
            val large = MaterialTheme.typography.headlineLarge.merge(NUMERAL).toSpanStyle()
            Text(
                buildAnnotatedString {
                    if (at < 0) {
                        append(sentence)
                    } else {
                        append(sentence.substring(0, at))
                        withStyle(large.copy(color = MaterialTheme.colorScheme.onSurface)) { append(number) }
                        append(sentence.substring(at + number.length))
                    }
                },
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .entrance(shown, Entrance.RISE, 120)
                    .clearAndSetSemantics { contentDescription = sentence },
            )
        }
        if (letter.shape.isNotEmpty()) {
            Column(
                modifier = Modifier.semantics(mergeDescendants = true) {}.entrance(shown, Entrance.RISE, 240),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(L10n.reveal_shape(ctx), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(letter.shape, style = MaterialTheme.typography.bodyLarge)
            }
        }
    }
}

@Composable
private fun Letter(letter: WritingStyleLetter, shown: Boolean) {
    val starts = letter.paragraphs.runningFold(0) { total, paragraph -> total + paragraph.lines.size }
    val serif = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Serif)
    val ink = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.14f)
    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.elevatedCardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLowest),
    ) {
        Column(modifier = Modifier.fillMaxWidth().padding(start = 18.dp, end = 18.dp, top = 16.dp, bottom = 18.dp)) {
            if (letter.opening.isNotEmpty()) {
                SpansText(
                    letter.opening,
                    MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif, fontWeight = FontWeight.Medium),
                    Modifier.padding(bottom = 12.dp).entrance(shown, Entrance.RISE, 40),
                )
            }
            Column(modifier = Modifier.clearAndSetSemantics {}, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                letter.paragraphs.forEachIndexed { paragraph, lines ->
                    Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
                        lines.lines.forEachIndexed { line, width ->
                            Spacer(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(7.dp)
                                    .growingBar(shown, width.toFloat(), ink, 3.5.dp, 200 + 60 * (starts[paragraph] + line)),
                            )
                        }
                    }
                }
            }
            if (letter.closing.isNotEmpty() || letter.signsAs.isNotEmpty()) {
                Column(modifier = Modifier.padding(top = 22.dp).entrance(shown, Entrance.RISE, 280 + 60 * starts.last())) {
                    if (letter.closing.isNotEmpty()) SpansText(letter.closing, serif)
                    if (letter.signsAs.isNotEmpty()) Text(letter.signsAs, style = serif)
                }
            }
        }
    }
}

/** Greetings and sign-offs, each over a bar as long as its part of its list, and the name signed with. */
@Composable
internal fun StyleHabitsView(habits: WritingStyleHabits, shown: Boolean) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        HabitGroup(L10n.reveal_greetings(ctx), habits.greetings, shown, 80)
        HabitGroup(L10n.reveal_sign_offs(ctx), habits.signOffs, shown, 320)
        if (habits.signsAs.isNotEmpty()) {
            Row(
                modifier = Modifier.semantics(mergeDescendants = true) {}.entrance(shown, Entrance.RISE, 560),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(L10n.reveal_signs_as(ctx), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(habits.signsAs, style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Serif))
            }
        }
    }
}

@Composable
private fun HabitGroup(label: String, habits: List<WritingStyleHabit>, shown: Boolean, startMillis: Int) {
    if (habits.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.semantics { heading() },
        )
        habits.forEachIndexed { index, habit -> HabitBar(habit, top = index == 0, shown, startMillis + 80 * index) }
    }
}

@Composable
private fun HabitBar(habit: WritingStyleHabit, top: Boolean, shown: Boolean, delayMillis: Int) {
    val ctx = LocalContext.current
    val colours = MaterialTheme.colorScheme
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .background(colours.surfaceContainerLow, RoundedCornerShape(12.dp))
            .growingBar(
                shown,
                habit.relative.toFloat() / 100f,
                if (top) colours.primaryContainer else colours.surfaceContainerHighest,
                12.dp,
                delayMillis,
            )
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SpansText(habit.runs, MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium), Modifier.weight(1f))
        Text(
            frequencyText(ctx, habit.frequency),
            style = MaterialTheme.typography.bodyMedium,
            color = if (top) colours.onPrimaryContainer else colours.onSurfaceVariant,
            modifier = Modifier.padding(start = 8.dp),
        )
    }
}

/** A greeting or sign-off, a placeholder such as Name drawn as a small pill in the accent. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SpansText(spans: List<WritingStyleSpan>, style: TextStyle, modifier: Modifier = Modifier) {
    if (spans.none { it.placeholder }) {
        Text(spans.joinToString("") { it.text }, style = style, modifier = modifier)
        return
    }
    val accent = MaterialTheme.colorScheme.primary
    FlowRow(modifier = modifier, itemVerticalAlignment = Alignment.CenterVertically) {
        spans.forEach { span ->
            if (span.placeholder) {
                Text(
                    span.text,
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = accent,
                    modifier = Modifier
                        .padding(horizontal = 2.dp)
                        .background(accent.copy(alpha = 0.1f), CircleShape)
                        .border(0.5.dp, accent.copy(alpha = 0.4f), CircleShape)
                        .padding(horizontal = 8.dp, vertical = 2.dp),
                )
            } else {
                Text(span.text, style = style)
            }
        }
    }
}
