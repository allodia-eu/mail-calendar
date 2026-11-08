// How an unanswered invitation looks on the calendar: a dashed border, a hatched leading gutter, and
// a fill that reads as provisional rather than booked.
//
// The core decides *which* records are holds (`participation == .needsAction`, docs/invitations.md);
// this file is only the drawing, kept in one place so the grid block, the all-day bar, the month chip
// and the invitation card's preview cannot drift apart.
//
// Every piece here is a no-op on an answered record, so nothing about a confirmed commitment's
// appearance changes: a hold is told apart by shape, not by a restyle of everything around it.
//
// The visual is never the whole disclosure. A dashed border is invisible to a screen reader, so every
// surface that draws a hold also says it, `calendarEventLabel` in InvitationFormat.swift appends
// "Awaiting your response" (docs/calendar.md §4, the spoken-grid rule).

import MailcalBindings
import SwiftUI

/// How much of a hold's colour survives. Enough to keep its calendar identifiable, little enough that
/// it does not read as a confirmed commitment beside one.
let holdFillOpacity: Double = 0.4

/// The border a record's edge is stroked with: dashed for an unanswered hold, the grid's own hairline
/// otherwise. Pure, so which shape a participation earns is a check that can fail.
func participationStroke(_ participation: ResponseStatus) -> StrokeStyle {
    isAwaitingResponse(participation)
        ? StrokeStyle(lineWidth: 1, dash: [3, 2])
        : StrokeStyle(lineWidth: 0.5)
}

/// The diagonal hatching down a hold's leading edge, the part of the treatment that survives being
/// looked at quickly, when a dashed border at a phone's hour height does not.
struct ParticipationHatch: View {
    let color: Color

    var body: some View {
        Canvas { context, size in
            context.stroke(holdHatchPath(size: size), with: .color(color), lineWidth: 1)
        }
    }
}

/// The hatch's stripes across a strip of `size`, shared by this view and the drawn grid.
func holdHatchPath(size: CGSize) -> Path {
    var path = Path()
    // Start a full height to the left so the first stripe already crosses the strip; step by 4pt so
    // the pattern stays legible at any block height.
    var x = -size.height
    while x < size.width + size.height {
        path.move(to: CGPoint(x: x, y: size.height))
        path.addLine(to: CGPoint(x: x + size.height, y: 0))
        x += 4
    }
    return path
}

extension View {
    /// A dashed border, on a hold only (`eventBlockStyle`); the drawn grid strokes the same dashes
    /// itself.
    @ViewBuilder
    func holdBorder(
        _ participation: ResponseStatus,
        color: Color,
        cornerRadius: CGFloat
    ) -> some View {
        if isAwaitingResponse(participation) {
            overlay {
                RoundedRectangle(cornerRadius: cornerRadius)
                    .strokeBorder(color, style: participationStroke(participation))
            }
        } else {
            self
        }
    }
}
