// How many all-day bars the banner shows before it starts hiding them, and how many it hid.
//
// The core stacks all-day and multi-day events into non-colliding **lanes** (in Rust, so no two
// clients can lane them differently) and reports the true lane count. It does NOT cap them: an
// uncapped banner grows a row per lane and, with a busy week, eats the grid it sits above.
//
// The cap is a client decision, it is a question of how much vertical room this screen has, the
// same category as the hour height, not a user setting the core must own. But the *rule* is shared,
// so it lives in one plain, tested function here and is written down in the calendar contract for
// Windows and Apple to copy.
package eu.allodia.mailcal

/**
 * Where one all-day bar sits: [days] columns from [day], stacked in [lane].
 *
 * Only the geometry, so the one tested overflow count below serves both the renderer (which has
 * colours and a spoken label attached, in [BandPaint]) and a test (which needs neither).
 */
internal data class BandSpan(val day: Int, val days: Int, val lane: Int)

/**
 * How many rows the banner shows when collapsed.
 *
 * With more lanes than this, the **last** visible row is given over to a per-day "+N" chip rather
 * than to a bar, so the banner is never taller than this, and no event is silently dropped.
 */
internal const val ALL_DAY_COLLAPSED_LANES = 3

/** The lanes that still hold real bars once a "+N" row is needed. */
internal const val ALL_DAY_VISIBLE_LANES = ALL_DAY_COLLAPSED_LANES - 1

/**
 * Whether there are more lanes than the collapsed banner can show.
 *
 * Exactly [ALL_DAY_COLLAPSED_LANES] lanes fit with no overflow row, the "+N" only appears when it
 * would actually be hiding something, so three all-day events show as three bars, not two and a
 * "+1".
 */
internal fun allDayOverflows(lanes: Int): Boolean = lanes > ALL_DAY_COLLAPSED_LANES

/** The lanes drawn as bars right now: all of them when expanded or when they fit, else the first
 *  [ALL_DAY_VISIBLE_LANES] with the last row reserved for the "+N" chips. */
internal fun allDayDrawnLanes(lanes: Int, expanded: Boolean): Int =
    if (expanded || !allDayOverflows(lanes)) lanes else ALL_DAY_VISIBLE_LANES

/** How tall the banner is, in lanes, including any "+N" row. */
internal fun allDayBannerLanes(lanes: Int, expanded: Boolean): Int =
    if (expanded || !allDayOverflows(lanes)) lanes else ALL_DAY_COLLAPSED_LANES

/**
 * For each day column, how many of its all-day bars the collapsed banner is hiding.
 *
 * A multi-day bar counts against **every** day it covers, it is hidden on all of them, so a
 * three-day offsite pushed out of view adds one to three different columns. Counting it once would
 * under-report two of them, and a "+1" that should say "+2" is a lie the user cannot see through.
 */
internal fun allDayOverflowPerDay(
    bands: List<BandSpan>,
    dayCount: Int,
    drawnLanes: Int,
): List<Int> = (0 until dayCount).map { day ->
    bands.count { band ->
        band.lane >= drawnLanes && day >= band.day && day < band.day + band.days
    }
}
