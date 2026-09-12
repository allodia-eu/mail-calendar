// The repeat summary as the user reads it, over the sentence parts the core decides.
//
// Which sentence a rule gets is pinned in the core (`repeat_summary_tests.rs`), once for every
// client. What is pinned here is the half that is genuinely this client's: that each arm reaches
// its catalog frame, that the platform's own weekday and month names are used and indexed by the
// right number, and that the ordinal picks the form the weekday agrees with.
//
// The agreement is asserted on the **selection**, not on a rendered Italian sentence: the catalog
// resolves through `Locale.preferredLanguages`, and this suite runs in parallel with every other,
// so overriding the process language to read Italian strings would decide what unrelated suites
// see. The words themselves are shared catalog data, rendered end to end by the Android suite.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventRepeatTextTests {
    private let english = Locale(identifier: "en_GB")

    private func summary(
        _ rhythm: RepeatRhythm,
        stop: RepeatStop = .never,
        locale: Locale? = nil
    ) -> String {
        recurrenceText(
            RepeatSummary(rhythm: rhythm, stop: stop),
            isRecurring: true,
            locale: locale ?? english
        )
    }

    /// The fourth `day` of every month.
    private func fourthOf(_ day: RecurrenceWeekday) -> RepeatRhythm {
        .monthlyOnWeekday(interval: 1, nth: 4, day: day)
    }

    @Test func aWeeklyRuleNamesItsWeekdays() {
        #expect(
            summary(.weekly(interval: 1, days: [.tuesday]))
                == L10n.event_repeat_sum_weekly(days: "Tuesday")
        )
        #expect(
            summary(.weekly(interval: 1, days: [.monday, .friday]))
                == L10n.event_repeat_sum_weekly(days: "Monday, Friday")
        )
    }

    @Test func everyWeekdayIsNamedByItsOwnName() {
        // The symbol list is indexed from Sunday and the core counts from Monday, so an off-by-one
        // here would rename every day of the week, and read perfectly plausibly while doing it.
        let all: [RecurrenceWeekday] = [
            .monday, .tuesday, .wednesday, .thursday, .friday, .saturday, .sunday,
        ]
        #expect(
            summary(.weekly(interval: 1, days: all))
                == L10n.event_repeat_sum_weekly(
                    days: "Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday"
                )
        )
    }

    @Test func aRuleThatSkipsPeriodsSaysHowMany() {
        let text = summary(.weekly(interval: 2, days: [.tuesday]))
        #expect(text == L10n.event_repeat_sum_weekly_n(count: 2, days: "Tuesday"))
        #expect(text.contains("2"))
        // A fortnightly rule must not read as the weekly one.
        #expect(text != L10n.event_repeat_sum_weekly(days: "Tuesday"))
    }

    @Test func aMonthlyRuleCountingAWeekdaysPositionSpellsThePositionOut() {
        #expect(
            summary(fourthOf(.monday))
                == L10n.event_repeat_sum_monthly_nth(
                    position: L10n.event_repeat_nth_fourth(weekday: "Monday")
                )
        )
    }

    @Test func anEndIsPartOfTheSentenceNotDroppedFromIt() {
        let until = summary(.daily(interval: 1), stop: .onDate(date: "2027-06-03"))
        #expect(until.contains("2027"))
        #expect(until.hasPrefix(L10n.event_repeat_daily()))

        #expect(
            summary(.daily(interval: 1), stop: .afterCount(count: 12))
                == L10n.event_repeat_sum_times(rule: L10n.event_repeat_daily(), count: 12)
        )
    }

    @Test func anEventWithNoSummarySaysItRepeatsAndOneWithNoRuleSaysItDoesNot() {
        // The core sends no summary for a rule it will not state exactly, the client must not
        // invent a rhythm for it, and must not call the event a one-off either.
        #expect(recurrenceText(nil, isRecurring: true, locale: english) == L10n.event_repeat_other())
        #expect(recurrenceText(nil, isRecurring: false, locale: english) == L10n.event_repeat_none())
    }

    @Test func theWeekdayAndMonthNamesComeFromTheDevicesLanguageNotFromOurs() {
        let dutch = Locale(identifier: "nl")
        #expect(summary(.weekly(interval: 1, days: [.tuesday]), locale: dutch).contains("dinsdag"))
        #expect(
            summary(.yearlyOnDate(interval: 1, month: 8, day: 25), locale: dutch)
                .contains("augustus")
        )
    }

    @Test func theAlternativeOrdinalIsChosenByIsoWeekdayNumber() {
        let all: [RecurrenceWeekday] = [
            .monday, .tuesday, .wednesday, .thursday, .friday, .saturday, .sunday,
        ]
        func alternativeForm(under entry: String) -> [RecurrenceWeekday] {
            let days = altWeekdays(entry)
            return all.filter { days.contains(isoWeekday($0)) }
        }
        // Italian inflects the ordinal for domenica alone; Portuguese for segunda through sexta.
        // Both sets are written as ISO numbers, so reading them against any other numbering, the
        // Sunday-first one Foundation uses for its symbols, say, inflects the wrong days.
        #expect(alternativeForm(under: "7") == [.sunday])
        #expect(alternativeForm(under: "1,2,3,4,5") == [.monday, .tuesday, .wednesday, .thursday, .friday])
        // The five languages where the question does not arise say nothing, and nothing inflects.
        #expect(alternativeForm(under: "") == [])
    }
}
