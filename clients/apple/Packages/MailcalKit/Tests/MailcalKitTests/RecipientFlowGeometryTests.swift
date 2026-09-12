// Where the recipient pills sit (RecipientFlowGeometry.swift).
//
// The rule every test here defends is one line long, **the container is never wider than the width
// it was given**, and breaking it does not look like a layout bug. A single address longer than
// the field made the flow report a container wider than the screen; the parent centred the
// overflow, so the whole composer slid off both edges and the pill's remove button landed 29 pt
// past the right edge, where nothing can tap it. Measured on a 402 pt iPhone, the "From" label sat
// at x = -55.

import CoreGraphics
import Testing

@testable import MailcalUI

@Suite struct RecipientFlowGeometryTests {
    /// A comfortable field width, and pills that are ordinary addresses at that width.
    private let limit: CGFloat = 400
    private let spacing: CGFloat = 6

    private func pill(_ width: CGFloat, _ height: CGFloat = 28) -> CGSize {
        CGSize(width: width, height: height)
    }

    @Test func oneAddressWiderThanTheFieldNeverWidensTheContainer() {
        // The regression. 472 pt of address in a 400 pt field: the container stays 400, and the
        // pill starts at the leading edge rather than being centred off it.
        let flow = recipientFlowGeometry(sizes: [pill(472)], limit: limit, spacing: spacing)
        #expect(flow.size.width == limit)
        #expect(flow.positions == [CGPoint(x: 0, y: 0)])
    }

    @Test func nothingIsEverPlacedOutsideTheContainer() {
        // Every origin inside, and every item's trailing edge inside, the property that makes the
        // remove button reachable, whatever mix of addresses is in the field.
        let sizes = [pill(120), pill(900), pill(60), pill(380), pill(1_200)]
        let flow = recipientFlowGeometry(sizes: sizes, limit: limit, spacing: spacing)
        for (origin, size) in zip(flow.positions, sizes) {
            #expect(origin.x >= 0)
            #expect(origin.x + min(size.width, limit) <= limit)
            #expect(origin.y >= 0)
            #expect(origin.y <= flow.size.height)
        }
        #expect(flow.size.width <= limit)
    }

    @Test func pillsThatFitShareALineAndTheNextOneWraps() {
        // Ordinary behaviour, unchanged: two 180 pt pills fit on a 400 pt line (180 + 6 + 180),
        // the third starts the next one.
        let sizes = [pill(180), pill(180), pill(180)]
        let flow = recipientFlowGeometry(sizes: sizes, limit: limit, spacing: spacing)
        #expect(flow.positions[0] == CGPoint(x: 0, y: 0))
        #expect(flow.positions[1] == CGPoint(x: 186, y: 0))
        #expect(flow.positions[2] == CGPoint(x: 0, y: 34))
        #expect(flow.size == CGSize(width: 366, height: 62))
    }

    @Test func theContainerIsAsTallAsEveryLineItWrapped() {
        // Height is the last line's bottom, not the tallest single pill: the field has to reserve
        // room for what it wrapped, or the rows beneath it are drawn over.
        let sizes = [pill(300), pill(300), pill(300)]
        let flow = recipientFlowGeometry(sizes: sizes, limit: limit, spacing: spacing)
        #expect(flow.size.height == 28 * 3 + spacing * 2)
    }

    @Test func aTallerPillSetsItsOwnLinesHeight() {
        // A wrapped address is two lines tall; the line it sits on grows, and the next line starts
        // below it rather than through it.
        let sizes = [pill(180, 28), pill(180, 44), pill(180, 28)]
        let flow = recipientFlowGeometry(sizes: sizes, limit: limit, spacing: spacing)
        #expect(flow.positions[2] == CGPoint(x: 0, y: 50))
        #expect(flow.size.height == 78)
    }

    @Test func anEmptyFieldTakesNoRoom() {
        // No pills yet is the composer's opening state, and a container with a height would push
        // the input down by a blank strip.
        let flow = recipientFlowGeometry(sizes: [], limit: limit, spacing: spacing)
        #expect(flow.size == .zero)
        #expect(flow.positions.isEmpty)
    }

    @Test func anUnboundedProposalKeepsEverythingOnOneLine() {
        // What a `nil` proposal width means: nothing constrains the line, so nothing wraps and the
        // clamp has nothing to clamp to.
        let sizes = [pill(472), pill(300)]
        let flow = recipientFlowGeometry(sizes: sizes, limit: .infinity, spacing: spacing)
        #expect(flow.positions[1] == CGPoint(x: 478, y: 0))
        #expect(flow.size.width == 778)
    }
}
