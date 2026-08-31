// The local date/time formatting helpers for the Android client. Split out of
// MainActivity.kt to keep each file under the 500-line limit. (Reply/Forward now open the
// shared rich composer in RichComposeScreen.kt, so no plain-text composer lives here.)
package eu.allodia.mailcal

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Whether times render on a 24-hour clock, the user's persisted **core** setting, not the device's.
 *
 * A CompositionLocal because it is genuinely ambient: mail rows, the reading header, the thread
 * strip and the calendar all render times, and threading a boolean through eight composable
 * signatures would be noise. Defaults to 24-hour, matching the core, so a composable that somehow
 * renders outside the provider still agrees with the rest of the app.
 */
internal val LocalUse24Hour = staticCompositionLocalOf { true }

/** The clock portion of a timestamp pattern. */
private fun clockPattern(use24Hour: Boolean): String = if (use24Hour) "HH:mm" else "h:mm a"

/**
 * Formats an engine timestamp as local date + time (no seconds). A `Z`-suffixed UTC
 * instant is rendered in [activeZoneId] (the active display zone), falling back to the
 * device default when the id is null/empty or unrecognized by Android's tzdata; a naive
 * wall-clock or bare date is shown as-is. SimpleDateFormat (not java.time) so no
 * core-library desugaring is needed.
 *
 * [use24Hour] has no default on purpose: it used to be hard-coded to 24-hour here while the
 * calendar read the *device's* clock setting, so one app rendered `14:05` in the message list and
 * `2 PM` on the grid. Forcing every call site to say which clock it wants is what stops that
 * drifting apart again.
 */
internal fun localDateTime(raw: String, activeZoneId: String?, use24Hour: Boolean): String {
    if (raw.isEmpty()) return ""
    if (raw.endsWith("Z")) {
        try {
            val parser = java.text.SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", java.util.Locale.US)
            parser.timeZone = java.util.TimeZone.getTimeZone("UTC")
            val instant = parser.parse(raw)
            if (instant != null) {
                val formatter = java.text.SimpleDateFormat(
                    "yyyy-MM-dd ${clockPattern(use24Hour)}",
                    java.util.Locale.US,
                )
                // Render in the active display zone; fall back to the device's local zone
                // until the first timezone snapshot has been pulled.
                if (!activeZoneId.isNullOrEmpty()) {
                    formatter.timeZone = resolveZone(activeZoneId)
                }
                return formatter.format(instant)
            }
        } catch (_: java.text.ParseException) {
            // fall through to the naive handling below
        }
    }
    return if (raw.length >= 16 && raw.contains("T")) {
        raw.take(16).replace("T", " ")
    } else {
        raw.take(10)
    }
}

/**
 * Formats an engine timestamp as a compact, Thunderbird-style relative label for a list row,
 * in [activeZoneId] and the device locale: today → time (`09:01`), the previous six days →
 * short weekday (`vr`, `do`), this year → day + month (`3 jul`), older → with the year. Falls
 * back to [localDateTime] for a naive/unparseable value. SimpleDateFormat + Calendar (not
 * java.time) so no core-library desugaring is needed, matching [localDateTime].
 */
internal fun relativeDate(raw: String, activeZoneId: String?, use24Hour: Boolean): String {
    if (raw.isEmpty() || !raw.endsWith("Z")) return localDateTime(raw, activeZoneId, use24Hour)
    val instant = try {
        java.text.SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", java.util.Locale.US).apply {
            timeZone = java.util.TimeZone.getTimeZone("UTC")
        }.parse(raw)
    } catch (_: java.text.ParseException) {
        null
    } ?: return localDateTime(raw, activeZoneId, use24Hour)

    val zone = if (!activeZoneId.isNullOrEmpty()) resolveZone(activeZoneId) else java.util.TimeZone.getDefault()
    val locale = java.util.Locale.getDefault()
    val msg = java.util.Calendar.getInstance(zone, locale).apply { time = instant }
    val now = java.util.Calendar.getInstance(zone, locale)
    val dayDiff = Math.round((startOfDay(now) - startOfDay(msg)) / 86_400_000.0).toInt()
    val sameYear = msg.get(java.util.Calendar.YEAR) == now.get(java.util.Calendar.YEAR)
    val pattern = relativeDatePattern(dayDiff, sameYear, use24Hour)
    return java.text.SimpleDateFormat(pattern, locale).apply { timeZone = zone }.format(instant)
}

/**
 * The date/time pattern a [relativeDate] label uses for a message [dayDiff] calendar days in the
 * past (0 = today), in [sameYear] as now: today → the clock, the previous six days → short weekday,
 * this year → day + month, older → with the year. Day 7 falls to the date on purpose, it is the
 * same weekday as today, so "Mon" for it would read as *this* Monday. Pure (no clock, no tz), so the
 * shared relative-label policy is unit-testable, see docs/timestamps.md, mirrored on Apple and
 * Windows by hand.
 */
internal fun relativeDatePattern(dayDiff: Int, sameYear: Boolean, use24Hour: Boolean): String = when {
    dayDiff == 0 -> clockPattern(use24Hour)
    dayDiff in 1..6 -> "EEE"
    sameYear -> "d MMM"
    else -> "d MMM yyyy"
}

/** Local-midnight epoch millis for [cal]'s zone, for a DST-safe calendar-day difference. */
private fun startOfDay(cal: java.util.Calendar): Long {
    val c = cal.clone() as java.util.Calendar
    c.set(java.util.Calendar.HOUR_OF_DAY, 0)
    c.set(java.util.Calendar.MINUTE, 0)
    c.set(java.util.Calendar.SECOND, 0)
    c.set(java.util.Calendar.MILLISECOND, 0)
    return c.timeInMillis
}

/**
 * Resolves an IANA id to a [java.util.TimeZone], falling back to the device default when
 * Android's tzdata doesn't recognise it. The core validates the active zone against the
 * engine's bundled tzdb (jiff), not Android's, so a core-supported id can be unknown here:
 * and `getTimeZone` silently returns GMT for any unknown id, which would wrongly render
 * every timestamp in GMT. We detect that (resolved id "GMT" for an id that isn't itself a
 * GMT/UTC/Etc zone) and use the device default instead.
 */
private fun resolveZone(activeZoneId: String): java.util.TimeZone {
    val tz = java.util.TimeZone.getTimeZone(activeZoneId)
    val looksLikeGmt = activeZoneId == "GMT" || activeZoneId == "UTC" || activeZoneId.startsWith("Etc/")
    return if (tz.id == "GMT" && !looksLikeGmt) java.util.TimeZone.getDefault() else tz
}
