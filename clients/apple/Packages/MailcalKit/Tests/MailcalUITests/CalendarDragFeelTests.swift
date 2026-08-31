// How a drag *feels*, as opposed to what it writes.
//
// The twin of Android's `CalendarDragFeelTest.kt`, case-for-case and name-for-name, §10 is a
// cross-platform contract, and two clients that agree in prose but not in arithmetic have not kept
// it. A drag is still a delta and the delta is still snapped (`CalendarDragTests` pins that). What
// this file pins is the gap deliberately opened between the block on screen and the number crossing
// the FFI:
//
//   - a press fills the **hour band it landed in**, and the slot is the union of that band and the
//     pointer, so nothing can jump;
//   - while the pointer is down the block follows it **between** snap steps, while the readout and
//     the write stay on the grid.
//
// Every assertion touching `preview()` or `moveArgs` is asserting the picture did not leak into the
// write.

import Foundation
import Testing

@testable import MailcalUI

/// A create drag: the pointer went down at `rawAnchor` and is now on `minute` / `raw`.
///
/// Built through `moved(toDay:minute:rawMinute:)` and with the anchor snapped here, exactly as
/// `calendarDrag(at:...)` does it, a state assembled by naming every field can hold a combination
/// the gesture layer cannot produce.
private func create(rawAnchor: Int, minute: Int? = nil, raw: Int? = nil) -> CalendarDragState {
    let snapped = snap(rawAnchor)
    let target = minute ?? snapped
    return CalendarDragState(
        kind: .create, subject: nil,
        anchorDay: 2, anchorMinute: snapped, day: 2, minute: snapped,
        rawMinute: rawAnchor, rawAnchorMinute: rawAnchor
    ).moved(toDay: 2, minute: target, rawMinute: raw ?? target)
}

private func snap(_ raw: Int) -> Int {
    Int((Double(raw) / Double(dragSnapMinutes)).rounded()) * dragSnapMinutes
}

private func subject(start: Int, end: Int) -> CalendarDragSubject {
    CalendarDragSubject(
        account: "a", event: "e", occurrenceStart: "",
        day: 2, startMinutes: start, endMinutes: end
    )
}

@Suite struct CalendarDragHourBandTests {

    @Test func aPressFillsTheHourItLandedIn() {
        // 10:15 is a touch, not a decision to meet at a quarter past.
        let early = create(rawAnchor: 615).preview()
        #expect(early.startMinutes == 600)
        #expect(early.endMinutes == 660)
    }

    @Test func theSlotAlwaysContainsThePointerThatAskedForIt() {
        // The defect this exists to hold: a press at 17:50 rounded to the *nearest* boundary and drew
        // 18:00–19:00, the whole block below the pointer. Every minute of the day must land in a
        // band containing it, so walk them all.
        for raw in 0..<dayMinutes {
            let slot = create(rawAnchor: raw).preview()
            #expect(
                raw >= slot.startMinutes && raw <= slot.endMinutes,
                "a press at \(raw) drew \(slot.startMinutes)..\(slot.endMinutes)"
            )
            #expect(slot.minutes == defaultCreateMinutes)
        }
    }

    @Test func aPressThatSnapsAcrossTheHourStillFillsTheBandItIsIn() {
        // 16:53 snaps forward to 17:00. Measuring the band from the snapped minute would draw
        // 17:00–18:00, the band *below* the pointer, the same defect by a different route.
        let slot = create(rawAnchor: 1013).preview()
        #expect(slot.startMinutes == 960)
        #expect(slot.endMinutes == 1020)
    }

    @Test func aPressInTheLastHourOfTheDayKeepsItsWholeHour() {
        let late = create(rawAnchor: 1430).preview()
        #expect(late.startMinutes == 1380)
        #expect(late.endMinutes == 1440)
    }

    @Test func draggingDownKeepsTheTopOnTheHour() {
        // The hand is setting the *end*; the start is the hour the press landed in, not the pointer.
        let down = create(rawAnchor: 1250, minute: 1290).preview()
        #expect(down.startMinutes == 1200)
        #expect(down.endMinutes == 1290)
    }

    @Test func draggingUpKeepsTheBottomOnTheHour() {
        let up = create(rawAnchor: 1250, minute: 1140).preview()
        #expect(up.startMinutes == 1140)
        #expect(up.endMinutes == 1260)
    }

    @Test func aSlotIsNeverShorterThanTheHourItBeganIn() {
        // The cost of the rule above, stated out loud: the union of a band and a point inside it is
        // the band, so a drag cannot draw anything shorter than an hour. Shorter is the editor's job.
        for pointer in 1200...1260 {
            let slot = create(rawAnchor: 1250, minute: pointer).preview()
            #expect(slot.startMinutes == 1200)
            #expect(slot.endMinutes == 1260)
        }
    }

    @Test func theSlotMovesContinuouslyAllTheWayThroughTheGesture() {
        // The flicker report, generalised: walk the pointer across the whole day, through the band it
        // started in and out the other side. Consecutive frames may never jump, which is why there
        // is no mode flag left to get wrong, because a union has no threshold to flip at.
        var previous = create(rawAnchor: 1250, minute: 0, raw: 0).livePreview()
        for pointer in 1...dayMinutes {
            let slot = create(rawAnchor: 1250, minute: pointer, raw: pointer).livePreview()
            #expect(
                abs(slot.startMinutes - previous.startMinutes) <= 1,
                "start jumped at \(pointer): \(previous.startMinutes) -> \(slot.startMinutes)"
            )
            #expect(
                abs(slot.endMinutes - previous.endMinutes) <= 1,
                "end jumped at \(pointer): \(previous.endMinutes) -> \(slot.endMinutes)"
            )
            previous = slot
        }
    }
}

@Suite struct CalendarDragSmoothTests {

    @Test func theLiveBlockMovesBetweenSnapStepsWhileTheWrittenOneDoesNot() {
        let drag = create(rawAnchor: 600, minute: 690, raw: 697)
        #expect(drag.livePreview().endMinutes == 697)  // the picture follows the pointer
        #expect(drag.preview().endMinutes == 690)  // the write stays on the grid
    }

    @Test func theAnchoredEdgeNeverDriftsOffTheGrid() {
        // Only the edge in the hand is smooth. The other is where the write will put it, so it must
        // not wander by a few points while the user watches.
        let drag = create(rawAnchor: 600, minute: 690, raw: 697)
        #expect(drag.livePreview().startMinutes == 600)
        #expect(drag.preview().startMinutes == 600)
    }

    @Test func aMoveCarriesTheRawDeltaInThePictureAndTheSnappedOneInTheWrite() {
        let moving = CalendarDragState(
            kind: .move, subject: subject(start: 600, end: 660),
            anchorDay: 2, anchorMinute: 600, day: 2, minute: 630, rawMinute: 637
        )
        #expect(moving.livePreview().startMinutes == 637)
        #expect(moving.livePreview().endMinutes == 697)
        #expect(moving.preview().startMinutes == 630)
        #expect(moving.preview().endMinutes == 690)
        #expect(moving.moveArgs(thisOccurrenceOnly: false)?.minutes == 30)
    }

    @Test func aResizeSmoothsOnlyTheEdgeInTheHand() {
        let resizing = CalendarDragState(
            kind: .resizeEnd, subject: subject(start: 600, end: 660),
            anchorDay: 2, anchorMinute: 660, day: 2, minute: 720, rawMinute: 713
        )
        #expect(resizing.livePreview().startMinutes == 600)
        #expect(resizing.livePreview().endMinutes == 713)
        #expect(resizing.preview().endMinutes == 720)
    }

    @Test func clampingHoldsTheSmoothEdgeToTheDayAsWell() {
        // The picture may be smooth; it may not show a block hanging off the end of the column,
        // because that is a write that cannot happen.
        let past = CalendarDragState(
            kind: .move, subject: subject(start: 1380, end: 1440),
            anchorDay: 2, anchorMinute: 1400, day: 2, minute: 1470, rawMinute: 1477
        ).clamped(toColumns: daysInWeek)
        #expect(past.livePreview().endMinutes == 1440)
        #expect(past.preview().endMinutes == 1440)
    }
}
