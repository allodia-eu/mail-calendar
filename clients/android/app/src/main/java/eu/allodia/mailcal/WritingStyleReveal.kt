// The reveal's steps and the arithmetic behind its pictures (docs/ai.md, "Learning" step 5). Plain
// values with no view in them, so the JVM suite pins them: which page shows one language, how many
// grey lines a miniature letter draws, and what a card says when the core gave it no heading.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import java.text.BreakIterator
import java.time.Instant
import java.time.ZoneId
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt
import uniffi.mailcal_bindings.AccountWritingStyleRow
import uniffi.mailcal_bindings.HabitFrequency
import uniffi.mailcal_bindings.WritingStyleRow

// The reveal's pages, in the order the sheet walks them.
internal enum class RevealStep {
    READ, LETTER, HABITS, VOICE, PHRASES, NAME;

    // Whether the page is about one language, which is what puts the language control on screen.
    val isPerLanguage: Boolean get() = this == LETTER || this == HABITS || this == VOICE || this == PHRASES
}

// Which of a stepped sheet's `count` pages is on screen. Compose state, so the footer and the pager
// follow it.
internal class WizardPager(count: Int) {
    val count = maxOf(count, 1)

    var index by mutableIntStateOf(0)
        private set

    val isFirst: Boolean get() = index == 0
    val isLast: Boolean get() = index == count - 1

    // Moves to `target`, held to the pages there are.
    fun go(target: Int) {
        index = target.coerceIn(0, count - 1)
    }
}

// The miniature letter's grey lines: one list per paragraph, each line's width as a share of the
// letter's. About fifteen words to a line, two paragraphs when the style does not say, and never
// more lines than the letter has room for, so a long typical reply still reads as a letter.
internal fun revealLetterLines(words: UInt, paragraphs: UInt): List<List<Double>> {
    val full = listOf(1.0, 0.96, 0.98)
    val last = listOf(0.58, 0.74, 0.44, 0.66)
    val count = if (paragraphs == 0u) 2 else minOf(paragraphs, 6u).toInt()
    val wanted = if (words == 0u) count * 2 else (words.toDouble() / 15).roundToInt()
    val lines = wanted.coerceIn(count, 12)
    return List(count) { paragraph ->
        val length = lines / count + if (paragraph < lines % count) 1 else 0
        List(length) { line -> if (line == length - 1) last[paragraph % last.size] else full[line % full.size] }
    }
}

// A card's heading and the text beneath it.
internal data class RevealCardText(val headline: String, val body: String)

// The core's heading goes over the whole description; without one, the description's first sentence
// becomes the heading and the rest stays beneath, so no sentence is shown twice.
internal fun revealCardText(headline: String, description: String, locale: Locale = Locale.ROOT): RevealCardText {
    val heading = headline.trim()
    val text = description.trim()
    if (heading.isNotEmpty()) return RevealCardText(heading, text)
    if (text.isEmpty()) return RevealCardText("", "")
    val sentences = BreakIterator.getSentenceInstance(locale).apply { setText(text) }
    val end = sentences.next().takeIf { it != BreakIterator.DONE } ?: text.length
    val sentence = text.substring(0, end).trim().removeSuffix(".")
    return RevealCardText(sentence, text.substring(end).trim())
}

// One run of a greeting or sign-off: words, or a bracketed placeholder such as `[Name]`, which the
// reveal draws as a pill because there is no name to put in its place.
internal data class RevealRun(val text: String, val placeholder: Boolean)

private val PLACEHOLDER = Regex("""\[([^\]\[]+)]""")

// `text` cut into runs, each placeholder apart from the words around it and without its brackets.
internal fun revealRuns(text: String): List<RevealRun> = buildList {
    var start = 0
    for (match in PLACEHOLDER.findAll(text)) {
        if (match.range.first > start) add(RevealRun(text.substring(start, match.range.first), false))
        add(RevealRun(match.groupValues[1], true))
        start = match.range.last + 1
    }
    if (start < text.length) add(RevealRun(text.substring(start), false))
}

// The word beside a greeting's or sign-off's bar. The core decides which, so every client agrees.
internal fun revealFrequencyText(ctx: Context, frequency: HabitFrequency): String = when (frequency) {
    HabitFrequency.MOSTLY -> L10n.reveal_frequency_mostly(ctx)
    HabitFrequency.OFTEN -> L10n.reveal_frequency_often(ctx)
    HabitFrequency.SOMETIMES -> L10n.reveal_frequency_sometimes(ctx)
}

// The "Since" figure: the day and month within this year, the month and year before it, because
// it is drawn at the size of the counts beside it and a full date does not fit there.
internal fun revealSinceText(seconds: Long, now: Instant, zone: ZoneId, locale: Locale): String {
    val thisYear = Instant.ofEpochSecond(seconds).atZone(zone).year == now.atZone(zone).year
    val format = android.icu.text.DateFormat.getInstanceForSkeleton(if (thisYear) "dMMMM" else "MMMy", locale)
    format.timeZone = android.icu.util.TimeZone.getTimeZone(zone.id)
    return format.format(Date(seconds * 1_000))
}

// The address of the account a style was learned from, while that account is still on this
// device; null for one that came from another device or whose account was removed.
internal fun revealSourceAddress(row: WritingStyleRow, accounts: List<AccountWritingStyleRow>): String? =
    row.sourceAccount.takeIf { it.isNotEmpty() }?.let { id -> accounts.firstOrNull { it.accountId == id }?.email }
