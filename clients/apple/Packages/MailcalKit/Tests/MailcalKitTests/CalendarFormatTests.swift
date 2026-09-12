// The calendar's localised copy, and the all-day overflow rule.
//
// The core emits ISO dates and wall-clock minutes and nothing else, so everything a user actually
// reads on the grid is assembled client-side, which means a bug here is a bug the user sees.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

private func calendar(_ zone: String = "Europe/Amsterdam") -> Calendar {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: zone)!
    calendar.locale = Locale(identifier: "en_GB")
    return calendar
}

private func days(_ iso: [String]) -> [Date] {
    iso.compactMap { parseISODate($0, calendar: calendar()) }
}

@Suite struct CalendarFormatTests {

    @Test func aWeekInsideOneMonthIsTitledWithThatMonth() {
        let week = days(["2026-07-06", "2026-07-08", "2026-07-12"])
        #expect(periodTitle(days: week, calendar: calendar(), locale: Locale(identifier: "en_GB")) == "Jul 2026")
    }

    @Test func aWeekStraddlingAMonthNamesBoth() {
        // Titling this week "July" is wrong for three of the columns on screen.
        let week = days(["2026-06-29", "2026-06-30", "2026-07-05"])
        let title = periodTitle(days: week, calendar: calendar(), locale: Locale(identifier: "en_GB"))
        #expect(title == "Jun – Jul 2026")
    }

    @Test func aWeekStraddlingAYearNamesBothYears() {
        let week = days(["2026-12-28", "2027-01-03"])
        let title = periodTitle(days: week, calendar: calendar(), locale: Locale(identifier: "en_GB"))
        #expect(title == "Dec 2026 – Jan 2027")
    }

    @Test func theGridFormatsAgainstTheAppLanguageNotTheHost() {
        // The whole Dutch date bug in one assertion. These formatters used to default to
        // `Locale.current`, which the app's Language setting cannot move, the bundle ships no
        // `.lproj`, so the OS resolves the app to English, and a Dutch build headed its calendar
        // "Jul 2026" and listed "Mon" under "zo". They must default to the language `L10n` itself
        // resolved (docs/timestamps.md).
        //
        // This fails on a revert because the two really are different values: the catalog resolves
        // a bare "en"/"nl", while `Locale.current` is region-qualified ("en_NL", "nl_NL").
        #expect(displayCalendar(zone: "Europe/Amsterdam").locale == L10n.appLocale)
    }

    @Test func theGridSpeaksDutchWhenTheAppDoes() {
        let dutch = Locale(identifier: "nl")
        let week = days(["2026-07-06", "2026-07-08", "2026-07-12"])
        #expect(periodTitle(days: week, calendar: calendar(), locale: dutch) == "jul 2026")
        #expect(monthTitle(week[0], calendar: calendar(), locale: dutch) == "juli 2026")
        #expect(weekdayShort(week[0], calendar: calendar(), locale: dutch) == "ma")
    }

    @Test func weekNumbersAreIso8601() {
        // The "wk 28" a Dutch or German user expects. ISO weeks start on Monday and belong to the year
        // holding their Thursday, so the turn of the year is where a naive implementation breaks, and
        // asking a US-locale calendar for a "week of year" gives a different number entirely.
        let zone = TimeZone(identifier: "Europe/Amsterdam")!
        #expect(isoWeekNumber(parseISODate("2026-07-06", calendar: calendar())!, zone: zone) == 28)
        #expect(isoWeekNumber(parseISODate("2026-07-12", calendar: calendar())!, zone: zone) == 28)
        #expect(isoWeekNumber(parseISODate("2026-07-13", calendar: calendar())!, zone: zone) == 29)
        // 2027-01-01 is a Friday, so it belongs to ISO week 53 of 2026, not week 1.
        #expect(isoWeekNumber(parseISODate("2027-01-01", calendar: calendar())!, zone: zone) == 53)
    }

    @Test func theHourRulerFollowsTheClockSetting() {
        #expect(hourLabel(9, use24Hour: true) == "09")
        #expect(hourLabel(23, use24Hour: true) == "23")
        #expect(hourLabel(9, use24Hour: false) == "9 AM")
        #expect(hourLabel(12, use24Hour: false) == "12 PM")
        #expect(hourLabel(23, use24Hour: false) == "11 PM")
        // Midnight is deliberately unlabelled: it would collide with the day heading above it.
        #expect(hourLabel(0, use24Hour: true) == "")
        #expect(hourLabel(0, use24Hour: false) == "")
    }

    @Test func clockTimesRoundTripTheAwkwardHours() {
        #expect(clockTime(570, use24Hour: true) == "09:30")
        #expect(clockTime(0, use24Hour: true) == "00:00")
        #expect(clockTime(1439, use24Hour: true) == "23:59")
        // Two traps in 12-hour: midnight is "12 AM", not "0 AM", and noon is "12 PM", not "0 PM".
        #expect(clockTime(0, use24Hour: false) == "12:00 AM")
        #expect(clockTime(750, use24Hour: false) == "12:30 PM")
        #expect(clockTime(570, use24Hour: false) == "9:30 AM")
        #expect(clockTime(1439, use24Hour: false) == "11:59 PM")
    }

    @Test func aBlocksSpokenTimeIsTheRangeItCovers() {
        #expect(timeRange(570, 585, use24Hour: true) == "09:30 – 09:45")
    }

    @Test func theDayAxisIsReadInTheDisplayZoneNotTheDevices() {
        // The core lays out in the DISPLAY zone. A client that parsed the day columns in the device's
        // zone would label them with different dates than the blocks were positioned against, which
        // shows up only when the two differ, i.e. exactly when the user is travelling and least able
        // to spot it.
        let amsterdam = calendar("Europe/Amsterdam")
        let tokyo = calendar("Asia/Tokyo")
        let inAmsterdam = parseISODate("2026-07-12", calendar: amsterdam)!
        let inTokyo = parseISODate("2026-07-12", calendar: tokyo)!
        // Different instants, the same civil date in two zones is not the same moment...
        #expect(inAmsterdam != inTokyo)
        // ...but each round-trips to the date it was named, in its own zone.
        #expect(isoDate(inAmsterdam, calendar: amsterdam) == "2026-07-12")
        #expect(isoDate(inTokyo, calendar: tokyo) == "2026-07-12")
    }
}

@Suite struct CalendarBlockLabelTests {

    @Test func aBlockNeverDrawsALabelItHasNoRoomFor() {
        // Regression from Android, and the rule that stops it recurring: a quarter-hour block is a few
        // points tall, and a label that does not fit gets cut through the middle. The grid was
        // geometrically perfect and looked broken.
        //
        // Swept across the whole zoom range, because the hour height is not a constant: what fits
        // pinched in does not fit zoomed out, and 15 minutes (the core's minimum segment) is the
        // shortest block that can exist.
        for hourHeight in [20.0, 32.0, 48.0, 64.0, 120.0, 200.0] {
            for minutes in [15, 20, 30, 45, 60, 120] {
                guard blockShowsLabel(minutes: minutes, hourHeight: hourHeight) else { continue }
                let space = blockLabelSpace(minutes: minutes, hourHeight: hourHeight)
                let line = blockLabelLineHeight(minutes: minutes)
                let needed = blockShowsTime(minutes: minutes, hourHeight: hourHeight) ? line * 2 : line
                #expect(
                    needed <= space,
                    "at \(hourHeight)/hour a \(minutes)-minute block draws a label it has no room for"
                )
            }
        }
    }

    @Test func zoomingInRevealsAShortEventsTitleAndZoomingOutHidesIt() {
        // What makes the rule above acceptable rather than a silent loss: a 15-minute event is a few
        // points tall at the whole-day zoom and cannot hold text, so it stays a coloured block:
        // keeping its spoken label, and zooming in brings the title back.
        #expect(blockShowsLabel(minutes: 15, hourHeight: 200), "pinched in, a standup shows its name")
        #expect(!blockShowsLabel(minutes: 15, hourHeight: 20), "zoomed out to the whole day, it cannot")
        #expect(blockShowsLabel(minutes: 60, hourHeight: 48), "an hour-long meeting reads at any zoom")
    }
}

@Suite struct CalendarAllDayTests {

    private func band(day: Int, days: Int, lane: Int, title: String = "e") -> AllDayBand {
        AllDayBand(
            account: "acct-1",
            event: "evt-\(title)-\(day)-\(lane)",
            calendar: "work",
            title: title,
            day: UInt32(day),
            days: UInt32(days),
            lane: UInt32(lane),
            continuesBefore: false,
            continuesAfter: false,
            canWrite: true,
            // These cases are about banner geometry; the occurrence token is pinned where it is
            // read, in `EventEditorStateTests`.
            occurrenceStart: "",
            participation: .accepted
        )
    }

    @Test func aBannerThatFitsShowsEverythingAndOffersNoExpand() {
        // Exactly three lanes fit. Showing two and a "+1" here would hide an event for no reason.
        for lanes in 0...allDayCollapsedLanes {
            #expect(!allDayOverflows(lanes: lanes))
            #expect(allDayDrawnLanes(lanes: lanes, expanded: false) == lanes)
            #expect(allDayBannerLanes(lanes: lanes, expanded: false) == lanes)
        }
    }

    @Test func pastTheCapTheLastRowBecomesTheMoreChip() {
        #expect(allDayOverflows(lanes: 5))
        #expect(allDayDrawnLanes(lanes: 5, expanded: false) == allDayVisibleLanes)
        #expect(allDayBannerLanes(lanes: 5, expanded: false) == allDayCollapsedLanes)
        // Expanding shows every lane.
        #expect(allDayDrawnLanes(lanes: 7, expanded: true) == 7)
    }

    @Test func aHiddenMultiDayBarCountsAgainstEveryDayItCovers() {
        // The trap. A three-day offsite pushed out of view is hidden on all three of its days, so it
        // must add one to each of their counts. Counting it once, on its first day, would leave two
        // columns quietly under-reporting, and a "+1" that should say "+2" is a lie the user cannot
        // see through.
        let bands = [
            band(day: 0, days: 7, lane: 0),
            band(day: 0, days: 7, lane: 1),
            band(day: 1, days: 3, lane: 2, title: "offsite"),
        ]
        let hidden = allDayOverflowPerDay(bands: bands, dayCount: 7, drawnLanes: allDayVisibleLanes)
        #expect(hidden == [0, 1, 1, 1, 0, 0, 0])
    }

    @Test func eachColumnReportsOnlyWhatItIsActuallyHiding() {
        // Monday hides two, Friday hides one, the rest hide nothing, a single global "+N" would be
        // wrong on every column but one.
        let bands = [
            band(day: 0, days: 7, lane: 0),
            band(day: 0, days: 7, lane: 1),
            band(day: 0, days: 1, lane: 2, title: "a"),
            band(day: 0, days: 1, lane: 3, title: "b"),
            band(day: 4, days: 1, lane: 2, title: "c"),
        ]
        let hidden = allDayOverflowPerDay(bands: bands, dayCount: 7, drawnLanes: allDayVisibleLanes)
        #expect(hidden == [2, 0, 0, 0, 1, 0, 0])
    }
}

@Suite struct CalendarMonthChipTests {

    @Test func aCellShowsEverythingItCanFit() {
        #expect(monthChipsShown(total: 3, capacity: 4) == 3)
        #expect(monthChipsShown(total: 4, capacity: 4) == 4)
        #expect(monthChipsShown(total: 0, capacity: 4) == 0)
    }

    @Test func theOverflowRowOnlyEarnsItsPlaceWhenItStandsForMoreThanItDisplaces() {
        // The subtlety. With capacity 4 and 5 events, drawing "+N more" COSTS a slot, so it draws 3
        // events and says "+2", hiding two to report two. Showing 4 and silently dropping one is not
        // an option, and "+1" would be a lie.
        let shown = monthChipsShown(total: 5, capacity: 4)
        #expect(shown == 3)
        #expect(5 - shown == 2, "the +N must count everything it is standing for")
        #expect(monthChipsShown(total: 20, capacity: 4) == 3)
        #expect(20 - monthChipsShown(total: 20, capacity: 4) == 17)
    }

    @Test func aCellTooSmallForAnyChipDrawsNoneRatherThanASliver() {
        #expect(monthChipCapacity(0) == 0)
        #expect(monthChipsShown(total: 3, capacity: 0) == 0)
        #expect(monthChipCapacity(60) >= 4)
    }
}
