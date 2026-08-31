// How many all-day bars the banner shows before it starts hiding them, and how many it hid.
//
// The core stacks all-day and multi-day events into non-colliding **lanes** (in Rust, so no two
// clients can lane them differently) and reports the true lane count. It does NOT cap them: an
// uncapped banner grows a row per lane and, on a busy week, eats the grid it sits above.
//
// The cap is a client decision, it is a question of how much vertical room this screen has, the same
// category as the hour height, not a user setting the core must own. But the *rule* is shared, so it
// lives in one plain, tested function here and mirrors the Android client exactly.

import MailcalBindings

/// How many rows the banner shows when collapsed. Past this, the last row becomes a per-day "+N" chip
/// rather than a bar, so the banner is never taller than this, and no event is silently dropped.
let allDayCollapsedLanes = 3

/// The lanes that still hold real bars once a "+N" row is needed.
let allDayVisibleLanes = allDayCollapsedLanes - 1

/// Whether there are more lanes than the collapsed banner can show.
///
/// Exactly `allDayCollapsedLanes` lanes fit with no overflow row, the "+N" only appears when it
/// would actually be hiding something, so three all-day events show as three bars, not two and a "+1".
func allDayOverflows(lanes: Int) -> Bool { lanes > allDayCollapsedLanes }

/// The lanes drawn as bars right now.
func allDayDrawnLanes(lanes: Int, expanded: Bool) -> Int {
    (expanded || !allDayOverflows(lanes: lanes)) ? lanes : allDayVisibleLanes
}

/// How tall the banner is, in lanes, including any "+N" row.
func allDayBannerLanes(lanes: Int, expanded: Bool) -> Int {
    (expanded || !allDayOverflows(lanes: lanes)) ? lanes : allDayCollapsedLanes
}

/// For each day column, how many of its all-day bars the collapsed banner is hiding.
///
/// A multi-day bar counts against **every** day it covers, it is hidden on all of them, so a
/// three-day offsite pushed out of view adds one to three different columns. Counting it once would
/// under-report two of them, and a "+1" that should say "+2" is a lie the user cannot see through:
/// they tap, find an event nobody told them about, and stop trusting the banner.
func allDayOverflowPerDay(bands: [AllDayBand], dayCount: Int, drawnLanes: Int) -> [Int] {
    (0..<dayCount).map { day in
        bands.filter { band in
            Int(band.lane) >= drawnLanes
                && day >= Int(band.day)
                && day < Int(band.day) + Int(band.days)
        }.count
    }
}
