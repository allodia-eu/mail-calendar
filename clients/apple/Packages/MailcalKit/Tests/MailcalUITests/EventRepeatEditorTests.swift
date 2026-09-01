// What the repeat controls send, and (more to the point) what they refuse to send.
//
// The rebuild itself is the core's and is tested there. What is this client's, and is tested here,
// is which of the three answers a save carries: nothing when the user never touched the repeat, no
// rule at all beside a single occurrence, and a settled scope question once the rule has moved.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventRepeatEditorTests {
    /// Tuesday 25 August 2026, 09:00: the event every editor below is opened on.
    private var startWall: String { "2026-08-25T09:00:00" }

    private func weeklyRule(interval: UInt32 = 1) -> SimpleRecurrence {
        SimpleRecurrence(
            frequency: .weekly,
            interval: interval,
            days: [RecurrenceDay(day: .tuesday, nth: nil)],
            monthDays: [],
            months: [],
            end: .never
        )
    }

    private func weeklyDraft(interval: UInt32 = 1) -> RepeatDraft {
        RepeatDraft(
            frequency: .weekly,
            interval: interval,
            weekdays: [.tuesday],
            end: .never,
            stored: weeklyRule(interval: interval)
        )
    }

    private func detail(
        isRecurring: Bool,
        recurrence: EventRecurrence?,
        repeatDraft: RepeatDraft?,
        occurrenceStart: String = ""
    ) -> EventDetail {
        EventDetail(
            account: "acct",
            key: "/cal/e.ics",
            calendar: "work",
            title: "Standup",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: startWall,
            end: "2026-08-25T09:30:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: recurrence,
            repeatSummary: nil,
            repeatDraft: repeatDraft,
            isRecurring: isRecurring,
            canWrite: true,
            occurrenceStart: occurrenceStart,
            attendees: []
        )
    }

    @Test func aSaveThatNeverTouchedTheRepeatSaysNothingAboutIt() {
        let editor = EventEditorState.edit(
            detail(
                isRecurring: true,
                recurrence: .simple(rule: weeklyRule()),
                repeatDraft: weeklyDraft()
            ),
            calendarName: "Work"
        )
        #expect(editor.updateArgs(thisOccurrenceOnly: false).recurrence == nil)
    }

    @Test func aChangedRepeatIsSentAsASet() {
        var editor = EventEditorState.edit(
            detail(
                isRecurring: true,
                recurrence: .simple(rule: weeklyRule()),
                repeatDraft: weeklyDraft()
            ),
            calendarName: "Work"
        )
        editor.repeatDraft?.interval = 2

        guard case .set(let rule) = editor.updateArgs(thisOccurrenceOnly: false).recurrence else {
            Issue.record("a changed repeat is a Set")
            return
        }
        #expect(rule.interval == 2)
        #expect(rule.days == [RecurrenceDay(day: .tuesday, nth: nil)])
    }

    @Test func choosingDoesNotRepeatClearsTheSeries() {
        var editor = EventEditorState.edit(
            detail(
                isRecurring: true,
                recurrence: .simple(rule: weeklyRule()),
                repeatDraft: weeklyDraft()
            ),
            calendarName: "Work"
        )
        editor.repeatDraft = nil
        #expect(editor.updateArgs(thisOccurrenceOnly: false).recurrence == .clear)
    }

    /// A rule belongs to the series. The core refuses the pairing, and the editor never builds it.
    @Test func aRuleNeverTravelsWithASingleOccurrence() {
        var editor = EventEditorState.edit(
            detail(
                isRecurring: true,
                recurrence: .simple(rule: weeklyRule()),
                repeatDraft: weeklyDraft(),
                occurrenceStart: "2026-09-01T09:00:00"
            ),
            calendarName: "Work"
        )
        editor.repeatDraft?.interval = 3

        let args = editor.updateArgs(thisOccurrenceOnly: true)
        #expect(args.occurrence == "2026-09-01T09:00:00")
        #expect(args.recurrence == nil)
    }

    /// Opened on one occurrence, a save normally asks which occurrences it meant. A changed rule
    /// answers that question on its own, so it is not put.
    @Test func aChangedRepeatSettlesTheScopeQuestion() {
        var editor = EventEditorState.edit(
            detail(
                isRecurring: true,
                recurrence: .simple(rule: weeklyRule()),
                repeatDraft: weeklyDraft(),
                occurrenceStart: "2026-09-01T09:00:00"
            ),
            calendarName: "Work"
        )
        #expect(editor.asksAboutTheSeries)

        editor.repeatDraft?.interval = 2
        #expect(!editor.asksAboutTheSeries)
    }

    /// A rule the core would not state is shown and not offered: the client never seeds an editor
    /// from a partial picture, because saving it back would drop the rest.
    @Test func aRuleTooRichToStateOffersNoControls() {
        let editor = EventEditorState.edit(
            detail(isRecurring: true, recurrence: .complex, repeatDraft: nil),
            calendarName: "Work"
        )
        #expect(!editor.canEditRepeat)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).recurrence == nil)
    }

    @Test func anEventThatDoesNotRepeatCanBeGivenARule() {
        var editor = EventEditorState.edit(
            detail(isRecurring: false, recurrence: nil, repeatDraft: nil),
            calendarName: "Work"
        )
        #expect(editor.canEditRepeat)
        #expect(editor.updateArgs(thisOccurrenceOnly: false).recurrence == nil)

        editor.repeatDraft = RepeatDraft(
            frequency: .daily, interval: 1, weekdays: [.tuesday], end: .never, stored: nil
        )
        guard case .set(let rule) = editor.updateArgs(thisOccurrenceOnly: false).recurrence else {
            Issue.record("a first rule is a Set")
            return
        }
        #expect(rule.frequency == .daily)
    }

    @Test func aCreateCarriesTheRuleAsAPlainRuleRatherThanAnAnswer() {
        var editor = EventEditorState.create(
            default: CalendarChoice(account: "acct", id: "work", name: "Work"),
            zone: "Europe/Amsterdam",
            now: EventEditorState.parseWall(startWall)
        )
        editor.title = "Standup"
        #expect(editor.createArgs().recurrence == nil)

        editor.repeatDraft = RepeatDraft(
            frequency: .weekly,
            interval: 2,
            weekdays: [.tuesday, .thursday],
            end: .afterCount(count: 8),
            stored: nil
        )
        let rule = editor.createArgs().recurrence
        #expect(rule?.frequency == .weekly)
        #expect(rule?.interval == 2)
        #expect(rule?.end == .afterCount(count: 8))
    }

    /// The row is drawn in the device's own week order, and a weekly rule is never left with no
    /// day: the core refuses one, and the last day ticked is what would produce it.
    @Test func theWeekdayRowStartsWhereTheDeviceStartsItsWeek() {
        #expect(localWeekOrder.count == 7)
        #expect(Set(localWeekOrder).count == 7)
        let first: RecurrenceWeekday = Calendar.current.firstWeekday == 1 ? .sunday : .monday
        if Calendar.current.firstWeekday == 1 || Calendar.current.firstWeekday == 2 {
            #expect(localWeekOrder.first == first)
        }
    }

    @Test func aRuleFirstChosenFallsOnTheEventsOwnWeekday() {
        // 25 August 2026 is a Tuesday.
        #expect(recurrenceWeekday(of: EventEditorState.parseWall(startWall)) == .tuesday)
    }
}
