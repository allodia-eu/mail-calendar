// The repeat rule as a sentence: "Every 2 weeks on Monday, Friday, until 3 Jun 2027".
//
// Wording only. The core decided which sentence the rule gets, it read the event's start for
// every part the rule leaves out, put the weekdays in week order, and dropped the rules it cannot
// state exactly, so this is a `when` over a closed set and a catalog lookup. Weekday and month
// names come from the platform's own locale data, the way the grid's headings and the detail's
// dates do (`CalendarFormat`), rather than from the catalog: they are the one part of a localized
// string we do not have to translate ourselves.
package eu.allodia.mailcal

import android.content.Context
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.Month
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.util.Locale
import uniffi.mailcal_bindings.RepeatRhythm
import uniffi.mailcal_bindings.RepeatStop
import uniffi.mailcal_bindings.RepeatSummary
import uniffi.mailcal_bindings.RecurrenceWeekday

/**
 * The repeat summary shown on an event's detail and in its editor.
 *
 * [summary] is `null` for an event with no rule, and for one whose rule the core would not state
 * exactly, those get the bare *Repeats*, because approximating states a series the user does not
 * have and nothing on screen would tell them apart.
 */
internal fun recurrenceSummary(
    ctx: Context,
    summary: RepeatSummary?,
    isRecurring: Boolean,
    locale: Locale,
): String {
    if (summary == null) {
        return if (isRecurring) L10n.event_repeat_other(ctx) else L10n.event_repeat_none(ctx)
    }
    val rhythm = rhythmText(ctx, summary.rhythm, locale)
    return when (val stop = summary.stop) {
        is RepeatStop.Never -> rhythm
        is RepeatStop.OnDate -> L10n.event_repeat_sum_until(
            ctx,
            rhythm,
            LocalDate.parse(stop.date)
                .format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale)),
        )
        is RepeatStop.AfterCount -> L10n.event_repeat_sum_times(ctx, rhythm, stop.count.toInt())
    }
}

/** The rhythm alone, without what ends it. */
private fun rhythmText(ctx: Context, rhythm: RepeatRhythm, locale: Locale): String {
    fun month(number: UInt) = Month.of(number.toInt()).getDisplayName(TextStyle.FULL, locale)
    return when (rhythm) {
        is RepeatRhythm.Daily ->
            if (rhythm.interval == 1u) {
                L10n.event_repeat_daily(ctx)
            } else {
                L10n.event_repeat_sum_daily_n(ctx, rhythm.interval.toInt())
            }

        is RepeatRhythm.Weekly -> {
            val days = rhythm.days.joinToString(", ") { weekdayName(it, locale) }
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_weekly(ctx, days)
            } else {
                L10n.event_repeat_sum_weekly_n(ctx, rhythm.interval.toInt(), days)
            }
        }

        is RepeatRhythm.MonthlyOnDay ->
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_monthly_day(ctx, rhythm.day.toString())
            } else {
                L10n.event_repeat_sum_monthly_day_n(
                    ctx,
                    rhythm.interval.toInt(),
                    rhythm.day.toString(),
                )
            }

        is RepeatRhythm.MonthlyOnLastDay ->
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_monthly_last(ctx)
            } else {
                L10n.event_repeat_sum_monthly_last_n(ctx, rhythm.interval.toInt())
            }

        is RepeatRhythm.MonthlyOnWeekday -> {
            val position = positionText(ctx, rhythm.nth, rhythm.day, locale)
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_monthly_nth(ctx, position)
            } else {
                L10n.event_repeat_sum_monthly_nth_n(ctx, rhythm.interval.toInt(), position)
            }
        }

        is RepeatRhythm.YearlyOnDate ->
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_yearly(ctx, rhythm.day.toString(), month(rhythm.month))
            } else {
                L10n.event_repeat_sum_yearly_n(
                    ctx,
                    rhythm.interval.toInt(),
                    rhythm.day.toString(),
                    month(rhythm.month),
                )
            }

        is RepeatRhythm.YearlyOnWeekday -> {
            val position = positionText(ctx, rhythm.nth, rhythm.day, locale)
            if (rhythm.interval == 1u) {
                L10n.event_repeat_sum_yearly_nth(ctx, position, month(rhythm.month))
            } else {
                L10n.event_repeat_sum_yearly_nth_n(
                    ctx,
                    rhythm.interval.toInt(),
                    position,
                    month(rhythm.month),
                )
            }
        }
    }
}

/**
 * "on the fourth Monday", "na quarta segunda-feira", the phrase both by-weekday sentences drop
 * into, **carrying its own article**.
 *
 * The article belongs here rather than in the frame because in some languages it has to agree with
 * the weekday, and the weekday is not known until this point. Italian inflects for *domenica* and
 * Portuguese for *segunda* through *sexta*; the rest of each language's weekdays take the other
 * form. So each position has two wordings, and **which weekdays take the alternative one is stated
 * in the catalog** (`event_repeat_nth_alt_days`, ISO weekday numbers) rather than as a table of
 * genders in here: it is a fact about a language, and it belongs beside that language's words.
 * A language where the question does not arise leaves the set empty and ships the same wording
 * twice.
 */
private fun positionText(ctx: Context, nth: Int, day: RecurrenceWeekday, locale: Locale): String {
    val weekday = weekdayName(day, locale)
    val alt = L10n.event_repeat_nth_alt_days(ctx)
        .split(',')
        .mapNotNull { it.trim().toIntOrNull() }
        .contains(isoWeekday(day).value)
    return when (nth) {
        1 -> if (alt) {
            L10n.event_repeat_nth_first_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_first(ctx, weekday)
        }
        2 -> if (alt) {
            L10n.event_repeat_nth_second_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_second(ctx, weekday)
        }
        3 -> if (alt) {
            L10n.event_repeat_nth_third_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_third(ctx, weekday)
        }
        4 -> if (alt) {
            L10n.event_repeat_nth_fourth_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_fourth(ctx, weekday)
        }
        5 -> if (alt) {
            L10n.event_repeat_nth_fifth_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_fifth(ctx, weekday)
        }
        else -> if (alt) {
            L10n.event_repeat_nth_last_alt(ctx, weekday)
        } else {
            L10n.event_repeat_nth_last(ctx, weekday)
        }
    }
}

/** The weekday's name in [locale], the platform's word for it, not one of ours. */
private fun weekdayName(day: RecurrenceWeekday, locale: Locale): String =
    isoWeekday(day).getDisplayName(TextStyle.FULL, locale)

/** The core's weekday as the JDK's, which is what carries the locale data. */
internal fun isoWeekday(day: RecurrenceWeekday): DayOfWeek = when (day) {
    RecurrenceWeekday.MONDAY -> DayOfWeek.MONDAY
    RecurrenceWeekday.TUESDAY -> DayOfWeek.TUESDAY
    RecurrenceWeekday.WEDNESDAY -> DayOfWeek.WEDNESDAY
    RecurrenceWeekday.THURSDAY -> DayOfWeek.THURSDAY
    RecurrenceWeekday.FRIDAY -> DayOfWeek.FRIDAY
    RecurrenceWeekday.SATURDAY -> DayOfWeek.SATURDAY
    RecurrenceWeekday.SUNDAY -> DayOfWeek.SUNDAY
}
