// The reveal's first three pages: what was read, a typical reply as a miniature letter, and the
// greetings and sign-offs as bars. Each page starts hidden and plays in once it is on screen; the
// arithmetic behind the pictures is in WritingStyleReveal.kt.
package eu.allodia.mailcal

import androidx.compose.animation.core.animateIntAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import java.time.Instant
import java.time.ZoneId
import uniffi.mailcal_bindings.HabitRow
import uniffi.mailcal_bindings.LanguageStyleRow
import uniffi.mailcal_bindings.WritingStyleDetail

private val NUMERAL = TextStyle(fontWeight = FontWeight.SemiBold, fontFeatureSettings = "tnum")

// Step 1: how many messages in how many languages since when, from which account.
@Composable
internal fun RevealReadPage(detail: WritingStyleDetail, address: String?, zone: ZoneId) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val shown = rememberShown()
    val codes = detail.languages.map { it.language }
    WizardPage(
        title = L10n.reveal_title(ctx),
        bottom = {
            Text(
                L10n.reveal_pages_intro(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.wizardEntrance(shown, WizardEntrance.RISE, 1_000),
            )
        },
    ) {
        address?.let {
            Text(
                L10n.reveal_based_on(ctx, address = it),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val oldest = detail.row.oldest
        val stats = buildList {
            add(RevealStat(L10n.reveal_stat_messages(ctx), detail.row.messages.toInt()))
            add(RevealStat(L10n.reveal_stat_languages(ctx), codes.size))
            oldest?.let { add(RevealStat(L10n.reveal_stat_since(ctx), 0, revealSinceText(it, Instant.now(), zone, locale))) }
        }
        val spoken = oldest?.let {
            L10n.reveal_stats_a11y(
                ctx,
                count = detail.row.messages.toInt(),
                date = epochDate(it, zone, locale),
                languages = writingStyleLanguages(ctx, codes),
            )
        }
        RevealStats(stats, shown, spoken)
        LanguageChips(codes, shown)
    }
}

// One figure of the first page: a count, or a date already worded.
private data class RevealStat(val label: String, val count: Int, val text: String? = null)

// The counts side by side, each under its label, at the largest of three sizes that fits the width.
@Composable
private fun RevealStats(stats: List<RevealStat>, shown: Boolean, spoken: String?) {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val label = MaterialTheme.typography.labelLarge
    val base = MaterialTheme.typography.displayMedium.merge(NUMERAL)
    BoxWithConstraints(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 8.dp)
            .then(if (spoken != null) Modifier.clearAndSetSemantics { contentDescription = spoken } else Modifier),
    ) {
        val limit = with(density) { maxWidth.toPx() }
        val gaps = with(density) { (STAT_GAP * 2 * (stats.size - 1)).toPx() }
        val sizes = listOf(1f, 0.78f, 0.6f).map { base.copy(fontSize = base.fontSize * it, lineHeight = base.lineHeight * it) }
        val numeral = sizes.firstOrNull { style ->
            stats.sumOf { stat ->
                maxOf(
                    measurer.measure(stat.label, label).size.width,
                    measurer.measure(stat.text ?: stat.count.toString(), style).size.width,
                )
            } + gaps <= limit
        } ?: sizes.last()
        Row(modifier = Modifier.height(IntrinsicSize.Min)) {
            stats.forEachIndexed { index, stat ->
                if (index > 0) {
                    Box(
                        modifier = Modifier
                            .padding(horizontal = STAT_GAP)
                            .width(1.dp)
                            .fillMaxHeight()
                            .background(MaterialTheme.colorScheme.outlineVariant),
                    )
                }
                Column(modifier = Modifier.wizardEntrance(shown, WizardEntrance.RISE, index * 120)) {
                    Text(stat.label, style = label, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (stat.text != null) {
                        Text(stat.text, style = numeral)
                    } else {
                        CountingText(stat.count, numeral, shown, index * 120)
                    }
                }
            }
        }
    }
}

private val STAT_GAP = 16.dp

// A count from nought, in a box already as wide as the final value so nothing beside it moves.
@Composable
private fun CountingText(target: Int, style: TextStyle, shown: Boolean, delayMillis: Int) {
    val value by animateIntAsState(if (shown) target else 0, tween(900, delayMillis, COUNT), label = "count")
    Box {
        Text(target.toString(), style = style, modifier = Modifier.alpha(0f))
        Text(value.toString(), style = style)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LanguageChips(codes: List<String>, shown: Boolean) {
    val ctx = LocalContext.current
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
                    .wizardEntrance(shown, WizardEntrance.POP, 760 + 80 * index)
                    .heightIn(min = 32.dp)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh, CircleShape)
                    .padding(start = 12.dp, end = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(modifier = Modifier.size(8.dp).background(colours[index % colours.size], CircleShape))
                Text(L10n.languageName(ctx, code), style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}

// Step 2: a typical reply as a miniature letter, above its length, shape and punctuation.
@Composable
internal fun RevealLetterPage(style: LanguageStyleRow) {
    val ctx = LocalContext.current
    val shown = rememberShown()
    WizardPage(title = L10n.reveal_step_letter(ctx)) {
        RevealLetter(style, shown)
        Text(
            L10n.reveal_letter_caption(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (style.typicalWords > 0u) {
            val count = style.typicalWords.toInt()
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
                    .padding(top = 8.dp)
                    .wizardEntrance(shown, WizardEntrance.RISE, 120)
                    .clearAndSetSemantics { contentDescription = sentence },
            )
        }
        RevealFact(L10n.reveal_shape(ctx), style.shape, shown, 240)
        RevealFact(L10n.reveal_punctuation(ctx), style.punctuation, shown, 360)
    }
}

@Composable
private fun RevealLetter(style: LanguageStyleRow, shown: Boolean) {
    val paragraphs = revealLetterLines(style.typicalWords, style.typicalParagraphs)
    val starts = paragraphs.runningFold(0) { total, lines -> total + lines.size }
    val serif = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Serif)
    val ink = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.14f)
    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.elevatedCardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLowest),
    ) {
        Column(modifier = Modifier.fillMaxWidth().padding(start = 18.dp, end = 18.dp, top = 16.dp, bottom = 18.dp)) {
            style.greetings.firstOrNull()?.let {
                RevealRunsText(
                    revealRuns(it.text),
                    MaterialTheme.typography.headlineSmall.copy(fontFamily = FontFamily.Serif, fontWeight = FontWeight.Medium),
                    Modifier.padding(bottom = 12.dp).wizardEntrance(shown, WizardEntrance.RISE, 40),
                )
            }
            Column(modifier = Modifier.clearAndSetSemantics {}, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                paragraphs.forEachIndexed { paragraph, widths ->
                    Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
                        widths.forEachIndexed { line, width ->
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
            val signOff = style.signOffs.firstOrNull()
            if (signOff != null || style.signsAs.isNotEmpty()) {
                Column(
                    modifier = Modifier
                        .padding(top = 22.dp)
                        .wizardEntrance(shown, WizardEntrance.RISE, 280 + 60 * starts.last()),
                ) {
                    signOff?.let { RevealRunsText(revealRuns(it.text), serif) }
                    if (style.signsAs.isNotEmpty()) Text(style.signsAs, style = serif)
                }
            }
        }
    }
}

@Composable
private fun RevealFact(label: String, value: String, shown: Boolean, delayMillis: Int) {
    if (value.isEmpty()) return
    Column(
        modifier = Modifier.semantics(mergeDescendants = true) {}.wizardEntrance(shown, WizardEntrance.RISE, delayMillis),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}

// Step 3: greetings and sign-offs, each over a bar as long as its part of its list.
@Composable
internal fun RevealHabitsPage(style: LanguageStyleRow) {
    val ctx = LocalContext.current
    val shown = rememberShown()
    WizardPage(title = L10n.reveal_step_habits(ctx)) {
        HabitGroup(L10n.reveal_greetings(ctx), style.greetings, shown, 80)
        HabitGroup(L10n.reveal_sign_offs(ctx), style.signOffs, shown, 320)
        if (style.signsAs.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .semantics(mergeDescendants = true) {}
                    .wizardEntrance(shown, WizardEntrance.RISE, 560),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    L10n.reveal_signs_as(ctx),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    style.signsAs,
                    style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Serif),
                )
            }
        }
    }
}

@Composable
private fun HabitGroup(label: String, habits: List<HabitRow>, shown: Boolean, startMillis: Int) {
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
private fun HabitBar(habit: HabitRow, top: Boolean, shown: Boolean, delayMillis: Int) {
    val ctx = LocalContext.current
    val shape = RoundedCornerShape(10.dp)
    val colours = MaterialTheme.colorScheme
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 40.dp)
            .background(colours.surfaceContainerLow, shape)
            .growingBar(
                shown,
                habit.relative.toFloat() / 100f,
                if (top) colours.primaryContainer else colours.surfaceContainerHighest,
                10.dp,
                delayMillis,
            )
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RevealRunsText(
            revealRuns(habit.text),
            MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
            Modifier.weight(1f),
        )
        Text(
            revealFrequencyText(ctx, habit.frequency),
            style = MaterialTheme.typography.bodyMedium,
            color = colours.onSurfaceVariant,
            modifier = Modifier.padding(start = 8.dp),
        )
    }
}

// A greeting or sign-off, a placeholder such as `[Name]` drawn as a small pill in the accent.
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun RevealRunsText(runs: List<RevealRun>, style: TextStyle, modifier: Modifier = Modifier) {
    if (runs.none { it.placeholder }) {
        Text(runs.joinToString("") { it.text }, style = style, modifier = modifier)
        return
    }
    val accent = MaterialTheme.colorScheme.primary
    FlowRow(modifier = modifier, itemVerticalAlignment = Alignment.CenterVertically) {
        runs.forEach { run ->
            if (run.placeholder) {
                Text(
                    run.text,
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = accent,
                    modifier = Modifier
                        .padding(horizontal = 1.dp)
                        .background(accent.copy(alpha = 0.1f), CircleShape)
                        .border(0.5.dp, accent.copy(alpha = 0.4f), CircleShape)
                        .padding(horizontal = 7.dp, vertical = 1.dp),
                )
            } else {
                Text(run.text, style = style)
            }
        }
    }
}
