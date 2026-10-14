// An invitee's answer from an event's detail, tested without a view: which button is the answer
// already given, that Maybe answers at once, that Decline asks first, and that nothing can be
// pressed while an answer is on its way.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventAnswerTests {
    private let all: [InvitationResponse] = [.accept, .tentative, .decline]

    @Test func onlyTheAnswerAlreadyGivenIsCurrent() {
        #expect(all.filter(EventAnswer(current: .accepted, saving: false).isCurrent) == [.accept])
        #expect(all.filter(EventAnswer(current: .tentative, saving: false).isCurrent) == [.tentative])
        #expect(all.filter(EventAnswer(current: .declined, saving: false).isCurrent) == [.decline])
        #expect(all.filter(EventAnswer(current: .needsAction, saving: false).isCurrent).isEmpty)
        #expect(all.filter(EventAnswer(current: .delegated, saving: false).isCurrent).isEmpty)
    }

    @Test func theCurrentAnswerCannotBePressedAgain() {
        let answer = EventAnswer(current: .accepted, saving: false)
        #expect(!answer.canPress(.accept))
        #expect(answer.press(.accept) == .none)
    }

    @Test func maybeAnswersAtOnce() {
        #expect(EventAnswer(current: .accepted, saving: false).press(.tentative) == .send(.tentative))
        #expect(EventAnswer(current: .needsAction, saving: false).press(.accept) == .send(.accept))
    }

    @Test func declineAsksFirstAndSendsNothingYet() {
        #expect(EventAnswer(current: .accepted, saving: false).press(.decline) == .confirmDecline)
    }

    @Test func nothingCanBePressedWhileAnAnswerIsOnItsWay() {
        let answer = EventAnswer(current: .needsAction, saving: true)
        for response in all {
            #expect(!answer.canPress(response))
            #expect(answer.press(response) == .none)
        }
    }

    @Test func theDetailOffersAnAnswerOnlyWhereTheCoreSetsOne() {
        // The organiser, an appointment and an account with no route to the organiser all arrive
        // with `myAnswer` unset.
        #expect(EventAnswer(detail: meeting(myAnswer: nil), saving: false) == nil)
        #expect(
            EventAnswer(detail: meeting(myAnswer: .tentative), saving: true)
                == EventAnswer(current: .tentative, saving: true)
        )
    }

    private func meeting(myAnswer: ResponseStatus?) -> EventDetail {
        EventDetail(
            account: "acct",
            key: "/cal/m.ics",
            calendar: "work",
            title: "Planning",
            allDay: false,
            timezone: "Europe/Amsterdam",
            start: "2026-08-03T10:00:00",
            end: "2026-08-03T11:00:00",
            location: nil,
            notes: nil,
            reminderMinutes: nil,
            recurrence: nil,
            repeatSummary: nil,
            repeatDraft: nil,
            isRecurring: false,
            canWrite: true,
            canEditInvitees: false,
            myAnswer: myAnswer,
            occurrenceStart: "",
            attendees: []
        )
    }
}
