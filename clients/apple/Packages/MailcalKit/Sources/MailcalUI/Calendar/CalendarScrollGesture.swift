// Wheel and trackpad scrolling for the time grid, the desktop's primary way to move a calendar,
// and the one input the grid did not read.
//
// The grid holds its own scroll offsets rather than living in a `ScrollView` (see CalendarGridView:
// a pinch has to MOVE them mid-gesture, which a ScrollView will not allow). That buys the anchored
// zoom and costs the scrolling a ScrollView would have given for free, so nothing but a one-finger
// DRAG moved the grid, and a two-finger trackpad scroll or a mouse wheel did nothing at all.
//
// This restores it the same way the pinch is read: a local `NSEvent` monitor, on a view that is
// never a hit-test target, so the blocks underneath keep every click they had. Both axes move, the
// wheel scrolls hours, a trackpad's sideways component scrolls days, and both clamp exactly where
// the drag clamps. **A week is still the page**: this pans within the week the pager is on, it never
// scrolls across the boundary into the next one (docs/calendar.md §"The days are one strip").

import SwiftUI

#if os(macOS)
    import AppKit

    /// Points to move per line for a device that reports in lines rather than pixels, a classic
    /// mouse wheel. A trackpad and a Magic Mouse send precise (pixel) deltas and skip this entirely.
    private let linesToPoints: CGFloat = 16

    /// Scrolls the grid from wheel/trackpad events, clamped to the same bounds as the drag.
    struct CalendarScrollGesture: ViewModifier {
        @Binding var dayOffset: CGFloat
        @Binding var hourOffset: CGFloat
        let maxDayOffset: CGFloat
        let maxHourOffset: CGFloat

        func body(content: Content) -> some View {
            content.overlay(ScrollCatcher(onScroll: apply))
        }

        /// AppKit's deltas are "how far the content should move", and these offsets are "how far the
        /// content has been moved *up and left*", so they subtract. macOS has already applied the
        /// user's natural-scrolling preference to the sign, so it is never second-guessed here.
        private func apply(_ deltaX: CGFloat, _ deltaY: CGFloat) {
            if deltaX != 0 {
                dayOffset = (dayOffset - deltaX).clamped(to: 0...maxDayOffset)
            }
            if deltaY != 0 {
                hourOffset = (hourOffset - deltaY).clamped(to: 0...maxHourOffset)
            }
        }
    }

    /// A transparent AppKit view that reports scroll deltas over the grid.
    ///
    /// Like `PinchCatcher`, it watches through a local monitor instead of claiming the events, so it
    /// never has to become a hit-test target and costs the content below nothing.
    private struct ScrollCatcher: NSViewRepresentable {
        let onScroll: (CGFloat, CGFloat) -> Void

        func makeCoordinator() -> Coordinator { Coordinator(self) }

        func makeNSView(context: Context) -> NSView {
            let view = PassthroughView()
            context.coordinator.watch(view)
            return view
        }

        func updateNSView(_ nsView: NSView, context: Context) {
            context.coordinator.parent = self
        }

        static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
            coordinator.stop()
        }

        /// Never a hit-test target, clicks on an event block still open it.
        final class PassthroughView: NSView {
            override func hitTest(_ point: NSPoint) -> NSView? { nil }
            override var isFlipped: Bool { true }
        }

        final class Coordinator {
            var parent: ScrollCatcher
            private weak var view: NSView?
            private var monitor: Any?

            init(_ parent: ScrollCatcher) { self.parent = parent }

            func watch(_ view: NSView) {
                self.view = view
                monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                    // Consumed only when it was ours; anything else travels on untouched, so the
                    // mailbox list and the sidebar keep scrolling normally.
                    (self?.handle(event) ?? false) ? nil : event
                }
            }

            func stop() {
                if let monitor { NSEvent.removeMonitor(monitor) }
                monitor = nil
            }

            /// Whether this scroll was over the grid, and was handled.
            private func handle(_ event: NSEvent) -> Bool {
                // The monitor sees every scroll in the app, so the grid takes only the ones aimed at
                // it: same window (a sheet is a different one), pointer over the grid.
                guard let view, let window = view.window, event.window === window else { return false }
                let point = view.convert(event.locationInWindow, from: nil)
                guard view.bounds.contains(point) else { return false }

                let scale = event.hasPreciseScrollingDeltas ? 1 : linesToPoints
                var deltaX = event.scrollingDeltaX * scale
                let deltaY = event.scrollingDeltaY * scale

                // Shift+wheel pans the days, the macOS convention, and the only way a plain mouse
                // (no horizontal axis to report) can reach the rest of a week at a narrow zoom. When
                // AppKit has already swapped the axes for this device, `deltaX` is non-zero and this
                // leaves it alone rather than swapping twice.
                if event.modifierFlags.contains(.shift), deltaX == 0 {
                    deltaX = deltaY
                    parent.onScroll(deltaX, 0)
                    return true
                }
                guard deltaX != 0 || deltaY != 0 else { return false }
                parent.onScroll(deltaX, deltaY)
                return true
            }
        }
    }
#endif
