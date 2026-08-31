// The shared relative-timestamp bucketing policy (docs/timestamps.md): today → the clock, the
// previous six days → short weekday, this year → day + month, older → day + month + year. The
// pattern selection is pure, so the policy Android and Windows re-implement by hand is checkable
// here too, the list row's date shrinks from a full timestamp to a compact label.

import Testing

@testable import MailcalUI

@Suite struct RelativeDateTests {

    @Test func todayIsTheClock() {
        #expect(relativeDatePattern(dayDiff: 0, sameYear: true) == "HH:mm")
    }

    @Test func thePreviousSixDaysAreTheShortWeekday() {
        for day in 1...6 {
            #expect(relativeDatePattern(dayDiff: day, sameYear: true) == "EEE")
        }
    }

    @Test func aWeekAgoFallsBackToTheDateSoAWeekdayIsNeverAmbiguous() {
        // Day 7 is the same weekday as today; "Mon" for it would read as *this* Monday.
        #expect(relativeDatePattern(dayDiff: 7, sameYear: true) == "d MMM")
    }

    @Test func anOlderYearCarriesTheYear() {
        #expect(relativeDatePattern(dayDiff: 400, sameYear: false) == "d MMM yyyy")
    }
}
