// Every localised string the calendar grid shows, assembled here.
//
// The Rust core emits machine-readable data only, ISO dates, wall-clock minutes, and owns no
// locale facility at all (AGENTS.md: "Localisation is client-side"). So the weekday headings, the
// hour ruler, the period title and the spoken labels are all built here, from the core's `days`
// list and the device locale. These are pure functions over plain values, so they are tested
// without a Context or a composition.
package eu.allodia.mailcal

import java.time.LocalDate
import java.time.format.TextStyle
import java.time.temporal.WeekFields
import java.util.Locale

/** The core hands day columns back as `YYYY-MM-DD`; parse without a formatter (ISO is the default). */
internal fun parseIsoDate(iso: String): LocalDate = LocalDate.parse(iso)

/** The abbreviated weekday for a column heading: "Mon", "ma". */
internal fun weekdayShort(date: LocalDate, locale: Locale): String =
    date.dayOfWeek.getDisplayName(TextStyle.SHORT, locale)

/** The ISO-8601 week number, the "WK 28" a Dutch or German user expects to see. */
internal fun isoWeekNumber(date: LocalDate): Int =
    date.get(WeekFields.ISO.weekOfWeekBasedYear())

/**
 * The title over the grid: the month (and year) the shown days fall in.
 *
 * A week straddling a month boundary names both, "Jun – Jul 2026", because titling it with just
 * one month is wrong for half the columns on screen. Straddling a *year* names both years too.
 */
internal fun periodTitle(days: List<LocalDate>, locale: Locale): String {
    if (days.isEmpty()) return ""
    val first = days.first()
    val last = days.last()
    val month = { d: LocalDate -> d.month.getDisplayName(TextStyle.SHORT, locale) }
    return when {
        first.year != last.year ->
            "${month(first)} ${first.year} – ${month(last)} ${last.year}"
        first.month != last.month ->
            "${month(first)} – ${month(last)} ${last.year}"
        else -> "${month(first)} ${first.year}"
    }
}

/**
 * The title over the month grid, the anchored month, not the days on screen.
 *
 * A month grid deliberately shows a few days of its neighbours, so titling it from its columns would
 * name June for a July page.
 */
internal fun monthTitle(anchor: LocalDate, locale: Locale): String =
    "${anchor.month.getDisplayName(TextStyle.FULL_STANDALONE, locale)} ${anchor.year}"

/**
 * An hour label for the ruler: "09" on a 24-hour device, "9 AM" on a 12-hour one.
 *
 * Midnight is not labelled, its label would collide with the day heading directly above it, and
 * the top gridline is unambiguous without one.
 */
internal fun hourLabel(hour: Int, use24Hour: Boolean): String = when {
    hour == 0 -> ""
    use24Hour -> "%02d".format(hour)
    hour < 12 -> "$hour AM"
    hour == 12 -> "12 PM"
    else -> "${hour - 12} PM"
}

/** Wall-clock minutes from midnight as a clock time: "09:30", or "9:30 AM". */
internal fun clockTime(minutes: Int, use24Hour: Boolean): String {
    val hour = (minutes / 60).coerceIn(0, 23)
    val minute = minutes % 60
    if (use24Hour) return "%02d:%02d".format(hour, minute)
    val suffix = if (hour < 12) "AM" else "PM"
    val twelve = when {
        hour == 0 -> 12
        hour > 12 -> hour - 12
        else -> hour
    }
    return "%d:%02d %s".format(twelve, minute, suffix)
}

/** The time a block spans, for its spoken label: "09:30 – 09:45". */
internal fun timeRange(startMinutes: Int, endMinutes: Int, use24Hour: Boolean): String =
    "${clockTime(startMinutes, use24Hour)} – ${clockTime(endMinutes, use24Hour)}"
