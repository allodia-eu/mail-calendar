// The invitation card's own rules, pinned as plain values (see InvitationFormat.swift).
//
// These are the parts a screenshot cannot check: that "0 other things" and "1 other things" never get
// printed, that the four attendee buckets all reach the line so the arithmetic adds up, that an
// all-day meeting's exclusive end reads as one date rather than two, and that the preview's hour span
// contains every block on the day, because a preview that clips is a preview that lies.
//
// Deliberately locale-pinned where a string is asserted: the point is *which* wording the rule picks,
// not how Dutch spells July (the Android suite's own trap, recorded in AGENTS.md).

import Foundation
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite struct InvitationFormatTests {

    private func tally(
        total: UInt32,
        accepted: UInt32 = 0,
        declined: UInt32 = 0,
        tentative: UInt32 = 0,
        needsAction: UInt32 = 0
    ) -> AttendeeTally {
        AttendeeTally(
            total: total,
            accepted: accepted,
            declined: declined,
            tentative: tentative,
            needsAction: needsAction
        )
    }

    // MARK: - Conflicts, in words

    @Test func conflictWordingHasItsOwnZeroAndOne() {
        // The three cases are distinct strings, not one template with a number in it: "0 other
        // things in your calendar" and "1 other things" are not sentences.
        let none = invitationConflictLine(count: 0, known: true)
        let one = invitationConflictLine(count: 1, known: true)
        let many = invitationConflictLine(count: 4, known: true)
        #expect(none != one)
        #expect(one != many)
        #expect(!none.contains("0"))
        #expect(many.contains("4"))
    }

    @Test func anUnreadCalendarDoesNotClaimTheDayIsFree() {
        // The regression this exists for: opening an invitation before the calendar had synced said
        // "Nothing else in your calendar then" over a diary nobody had read, and the day in fact had
        // two clashes on it. Unknown is a THIRD sentence, never the zero one.
        let unknown = invitationConflictLine(count: 0, known: false)
        #expect(unknown != invitationConflictLine(count: 0, known: true))
        // A count that arrived anyway must not leak into the wording.
        #expect(invitationConflictLine(count: 7, known: false) == unknown)
        #expect(!unknown.contains("7"))
    }

    // MARK: - The attendee tally

    @Test func anInvitationWithOnlyMeSaysSo() {
        // "1 of 1 accepted" is technically true and reads as a committee of one.
        #expect(invitationAttendeeLines(tally(total: 1, accepted: 1)) == [L10n.invitation_attendees_one()])
    }

    @Test func noAttendeesAtAllProducesNoLine() {
        #expect(invitationAttendeeLines(tally(total: 0)).isEmpty)
    }

    @Test func everyNonZeroBucketReachesTheLine() {
        // The four buckets sum to the total, so a bucket left out is arithmetic that does not add up.
        let lines = invitationAttendeeLines(
            tally(total: 6, accepted: 2, declined: 1, tentative: 1, needsAction: 2)
        )
        #expect(lines.count == 4)
        #expect(lines[0].contains("2") && lines[0].contains("6"))
        #expect(lines[1].contains("1"))
        #expect(lines[2].contains("1"))
        #expect(lines[3].contains("2"))
    }

    @Test func emptyBucketsAreLeftOut() {
        // Nobody has declined, so the card does not say "0 declined".
        let lines = invitationAttendeeLines(tally(total: 3, accepted: 3))
        #expect(lines.count == 1)
    }

    // MARK: - The "when" line

    @Test func aTimedMeetingNamesItsDateOnce() {
        let line = invitationWhen(
            startsAt: "2026-07-30T14:30:00Z",
            endsAt: "2026-07-30T15:00:00Z",
            allDay: false,
            zone: "Europe/Amsterdam",
            use24Hour: true,
            locale: Locale(identifier: "en_GB")
        )
        // 14:30 UTC is 16:30 in Amsterdam in July, the zone is applied, not ignored.
        #expect(line.contains("16:30"))
        #expect(line.contains("17:00"))
        #expect(line.contains("2026"))
        // One date for a meeting inside one day.
        #expect(line.filter { $0 == "–" }.count == 1)
    }

    @Test func theTwelveHourSettingIsHonoured() {
        // The user's 12/24-hour setting, not the locale's default: mail and calendar must agree.
        let line = invitationWhen(
            startsAt: "2026-07-30T14:30:00Z",
            endsAt: "2026-07-30T15:00:00Z",
            allDay: false,
            zone: "Europe/Amsterdam",
            use24Hour: false,
            locale: Locale(identifier: "en_GB")
        )
        #expect(line.contains("4:30 PM"))
        #expect(!line.contains("16:30"))
    }

    @Test func aOneDayAllDayMeetingReadsAsOneDate() {
        // The stored end is EXCLUSIVE, next midnight, so the inclusive last day is the start day.
        let line = invitationWhen(
            startsAt: "2026-07-29T22:00:00Z",
            endsAt: "2026-07-30T22:00:00Z",
            allDay: true,
            zone: "Europe/Amsterdam",
            use24Hour: true,
            locale: Locale(identifier: "en_GB")
        )
        #expect(!line.contains("–"))
        #expect(line.contains("30"))
    }

    @Test func aMultiDayAllDayMeetingNamesBothEnds() {
        let line = invitationWhen(
            startsAt: "2026-07-29T22:00:00Z",
            endsAt: "2026-08-01T22:00:00Z",
            allDay: true,
            zone: "Europe/Amsterdam",
            use24Hour: true,
            locale: Locale(identifier: "en_GB")
        )
        #expect(line.contains("–"))
        // 30 July through 1 August inclusive, never the exclusive 2nd.
        #expect(line.contains("30"))
        #expect(!line.contains(" 2 "))
    }

    @Test func anUnparseableInstantYieldsNoLineRatherThanAWrongOne() {
        #expect(
            invitationWhen(
                startsAt: "",
                endsAt: "",
                allDay: false,
                zone: "Europe/Amsterdam",
                use24Hour: true
            ).isEmpty
        )
    }

    // MARK: - The preview's hour span

    @Test func theSpanContainsEveryClashInFull() {
        // The one thing that may not fall outside the band. A conflict is by definition an event
        // overlapping the meeting, and it has to be drawn *whole*, a long booking cut off at the
        // top edge loses its title with it, which is exactly what the band exists to show.
        let span = invitationPreviewSpan(
            meeting: MinuteSpan(start: 14 * 60, end: 15 * 60),
            others: [MinuteSpan(start: 9 * 60, end: 16 * 60)]
        )
        #expect(span.lowerBound <= 9)
        #expect(span.upperBound >= 16)
    }

    @Test func theSpanLeavesOutTheRestOfTheDay() {
        // …and everything that does NOT clash is left out, which is what buys the hours their
        // height. The card states the count in words above the grid and the disclosure label says
        // "around this meeting", so nothing is hidden without saying so.
        let span = invitationPreviewSpan(
            meeting: MinuteSpan(start: 14 * 60, end: 15 * 60),
            others: [MinuteSpan(start: 8 * 60, end: 9 * 60), MinuteSpan(start: 21 * 60, end: 22 * 60)]
        )
        #expect(!span.contains(8))
        #expect(!span.contains(21))
        #expect(span.contains(14))
    }

    @Test func aBlockEndingAsTheMeetingBeginsIsNotAClash() {
        // Half-open on both sides, exactly as the core's conflict count overlaps: back-to-back is
        // how a diary is packed, and widening the band for it would undo the zoom on every meeting
        // that follows another.
        let span = invitationPreviewSpan(
            meeting: MinuteSpan(start: 14 * 60, end: 15 * 60),
            others: [MinuteSpan(start: 6 * 60, end: 14 * 60)]
        )
        #expect(!span.contains(6))
    }

    @Test func theMeetingAloneStillGetsAReadableSpan() {
        // A 30-minute meeting on an empty day would otherwise be a two-hour sliver with no context.
        let span = invitationPreviewSpan(meeting: MinuteSpan(start: 600, end: 630), others: [])
        #expect(span.count >= 6)
        #expect(span.contains(10))
    }

    @Test func theBandKeepsTheMeetingAwayFromItsEdges() {
        // Padding grown alternately, not all onto one end: a meeting pinned to the top of its own
        // preview reads as if the day starts there.
        let span = invitationPreviewSpan(meeting: MinuteSpan(start: 14 * 60, end: 15 * 60), others: [])
        #expect(span.lowerBound < 14)
        #expect(span.upperBound > 15 + 1)
    }

    @Test func theSpanNeverLeavesTheDay() {
        // Padding and the minimum both push against the ends of the day, and the band is what the
        // preview divides its height by, an hour outside 0..<24 would place blocks off the grid.
        let firstThing = invitationPreviewSpan(meeting: MinuteSpan(start: 0, end: 30), others: [])
        #expect(firstThing.lowerBound == 0)
        #expect(firstThing.upperBound <= 24)
        let lastThing = invitationPreviewSpan(
            meeting: MinuteSpan(start: 23 * 60, end: 24 * 60),
            others: []
        )
        #expect(lastThing.lowerBound >= 0)
        #expect(lastThing.upperBound == 24)
        // Both still cover the whole minimum, growing inwards when one end has run out of room.
        #expect(firstThing.count >= 6)
        #expect(lastThing.count >= 6)
    }

    @Test func aBlockEndingMidHourKeepsThatWholeHour() {
        // Ceil, not floor: a block ending at 09:15 that lost the 09:00 hour would be drawn cut off.
        let span = invitationPreviewSpan(meeting: MinuteSpan(start: 480, end: 555), others: [])
        #expect(span.upperBound >= 10)
    }

    @Test func theRulerThinsOutWhenAnHourIsTooShortToLabel() {
        #expect(invitationPreviewStride(hourHeight: 25) == 1)
        #expect(invitationPreviewStride(hourHeight: 18) == 1)
        // A 24-hour span squeezed into the preview's height: label every third hour, not every one.
        #expect(invitationPreviewStride(hourHeight: 150 / 24) > 1)
    }

    // MARK: - The preview's height

    @Test func everyBandTheSpanCanProduceCanNameAOneHourBlock() {
        // The one thing the preview has to say. The band and the box are two halves of one rule:
        // narrow the band, or grow the box, and only their *ratio* decides whether a block gets a
        // title. So compose them, rather than pinning either number: a change to the span, the
        // height, or the label threshold has to keep all three compatible.
        for hours in 6...12 {
            let hourHeight = invitationPreviewHeight(hours: hours) / CGFloat(hours)
            #expect(
                blockShowsLabel(minutes: 60, hourHeight: hourHeight),
                "a one-hour block must carry its title over a \(hours)-hour band"
            )
        }
    }

    @Test func theBoxOnlyGrowsWhenTheBandCannotStayNarrow() {
        // The ordinary case is the plain height: the band is six hours, so there is nothing to fix.
        #expect(invitationPreviewHeight(hours: 6) == 150)
        // A long booking the meeting sits inside forces a wider band; the box follows it…
        #expect(invitationPreviewHeight(hours: 10) > invitationPreviewHeight(hours: 6))
        // …but stops, rather than pushing the message itself off the screen.
        #expect(invitationPreviewHeight(hours: 24) == 240)
    }

    // MARK: - The meeting's own window, in the layout zone

    @Test func theMeetingWindowIsWallClockInTheLayoutZone() {
        let span = meetingMinuteSpan(
            startsAt: "2026-07-30T14:30:00Z",
            endsAt: "2026-07-30T15:00:00Z",
            zone: "Europe/Amsterdam"
        )
        #expect(span == MinuteSpan(start: 16 * 60 + 30, end: 17 * 60))
    }

    @Test func aMeetingRunningPastMidnightEndsAtTheBottomOfTheDay() {
        let span = meetingMinuteSpan(
            startsAt: "2026-07-30T21:00:00Z",
            endsAt: "2026-07-31T01:00:00Z",
            zone: "Europe/Amsterdam"
        )
        #expect(span.start == 23 * 60)
        #expect(span.end == 24 * 60)
    }

    // MARK: - The spoken label

    @Test func anUnansweredHoldSaysSoOutLoud() {
        // The dashed border and hatched gutter are invisible to a screen reader (docs/calendar.md §4).
        let answered = calendarEventLabel(
            title: "Standup",
            time: "09:00 – 09:15",
            calendar: "Work",
            participation: .accepted
        )
        let hold = calendarEventLabel(
            title: "Standup",
            time: "09:00 – 09:15",
            calendar: "Work",
            participation: .needsAction
        )
        #expect(!answered.contains(L10n.a11y_invitation_awaiting_response()))
        #expect(hold.contains(L10n.a11y_invitation_awaiting_response()))
        #expect(hold.hasPrefix(answered))
    }

    @Test func onlyAnUnansweredRecordIsDrawnAsAHold() {
        #expect(isAwaitingResponse(.needsAction))
        #expect(!isAwaitingResponse(.accepted))
        #expect(!isAwaitingResponse(.tentative))
        #expect(!isAwaitingResponse(.delegated))
        // Declined never reaches a client at all, the core hides those from every surface.
        #expect(!isAwaitingResponse(.declined))
    }

    @Test func onlyAHoldIsDashed() {
        #expect(participationStroke(.needsAction).dash == [3, 2])
        #expect(participationStroke(.accepted).dash.isEmpty)
    }

    // MARK: - Titles

    @Test func eachKindGetsItsOwnHeading() {
        // A cancellation must not read as an invitation still waiting for an answer, and an
        // out-of-date copy must not read as either.
        let titles = [
            invitationTitle(.rsvp),
            invitationTitle(.cancelled),
            invitationTitle(.informational),
            invitationTitle(.superseded),
        ]
        #expect(Set(titles).count == titles.count)
    }

    @Test func onlyASupersededCardExplainsItself() {
        // The other three either offer buttons or say plainly what they are. A superseded card
        // still *looks* answerable, so its missing buttons are the one absence needing a sentence.
        #expect(invitationNotice(.superseded) != nil)
        #expect(invitationNotice(.rsvp) == nil)
        #expect(invitationNotice(.cancelled) == nil)
        #expect(invitationNotice(.informational) == nil)
    }

    @Test func eachAnswerGetsItsOwnReplySubjectNamingTheMeeting() {
        // This one leaves the device: on an account whose calendar server does no scheduling, the
        // core emails the reply itself and this is the subject line the organiser reads. Three
        // answers sharing a subject would have them guessing which way we answered, and a subject
        // without the summary would leave them guessing which meeting.
        let subjects = [
            invitationReplySubject(.accept, "Sprint planning"),
            invitationReplySubject(.tentative, "Sprint planning"),
            invitationReplySubject(.decline, "Sprint planning"),
        ]
        #expect(Set(subjects).count == subjects.count)
        #expect(subjects.allSatisfy { $0.contains("Sprint planning") })
    }

    @Test func everyResponseStatusHasWording() {
        let lines = [
            invitationResponseLine(.needsAction),
            invitationResponseLine(.accepted),
            invitationResponseLine(.declined),
            invitationResponseLine(.tentative),
            invitationResponseLine(.delegated),
        ]
        #expect(Set(lines).count == 5)
        #expect(!lines.contains(""))
    }
}
