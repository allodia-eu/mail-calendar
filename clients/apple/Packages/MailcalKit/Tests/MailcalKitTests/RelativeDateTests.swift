// The shared relative-timestamp bucketing policy (docs/timestamps.md): today → the clock, the
// previous six days → short weekday, this year → day + month, older → day + month + year. The
// pattern selection is pure, so the policy Android and Windows re-implement by hand is checkable
// here too, the list row's date shrinks from a full timestamp to a compact label.

import Testing

@testable import MailcalUI

@Suite struct RelativeDateTests {

    @Test func todayIsTheClock() {
        #expect(relativeDatePattern(dayDiff: 0, sameYear: true, use24Hour: true) == "HH:mm")
        #expect(relativeDatePattern(dayDiff: 0, sameYear: true, use24Hour: false) == "h:mm a")
    }

    /// The clock setting reaches the reading header, and through it the quoted original, in the
    /// calendar's own words, so mail and the grid never disagree about 14:05 and 2:05 PM.
    @MainActor @Test func theAbsoluteDateFollowsTheClockSettingAsTheCalendarDoes() {
        let raw = "2026-07-20T14:05:00Z"
        #expect(localDateTime(raw, in: "UTC", use24Hour: true) == "2026-07-20 14:05")
        #expect(localDateTime(raw, in: "UTC", use24Hour: false) == "2026-07-20 2:05 PM")
        #expect(
            localDateTime(raw, in: "UTC", use24Hour: false)
                .hasSuffix(clockTime(14 * 60 + 5, use24Hour: false))
        )
        // A floating time, an agenda event's, is on the same clock.
        #expect(localDateTime("2026-07-20T14:05:00", in: "UTC", use24Hour: false) == "2026-07-20 2:05 PM")
        #expect(localDateTime("2026-07-20T14:05:00", in: "UTC", use24Hour: true) == "2026-07-20 14:05")
    }

    @Test func thePreviousSixDaysAreTheShortWeekday() {
        for day in 1...6 {
            #expect(relativeDatePattern(dayDiff: day, sameYear: true, use24Hour: true) == "EEE")
        }
    }

    @Test func aWeekAgoFallsBackToTheDateSoAWeekdayIsNeverAmbiguous() {
        // Day 7 is the same weekday as today; "Mon" for it would read as *this* Monday.
        #expect(relativeDatePattern(dayDiff: 7, sameYear: true, use24Hour: true) == "d MMM")
    }

    @Test func anOlderYearCarriesTheYear() {
        #expect(relativeDatePattern(dayDiff: 400, sameYear: false, use24Hour: true) == "d MMM yyyy")
    }
}
