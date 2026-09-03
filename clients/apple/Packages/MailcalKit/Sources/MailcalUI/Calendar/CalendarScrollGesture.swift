// Wheel and trackpad scrolling for the time grid: the desktop's primary way to move a calendar, and
// the iPad's, once a keyboard case is attached.
//
// The grid holds its own scroll offsets rather than living in a `ScrollView` (see CalendarGridView:
// a pinch has to MOVE them mid-gesture, which a ScrollView will not allow). That buys the anchored
// zoom and costs the scrolling a ScrollView would have given for free, so this puts it back, by
// reading the raw events on each platform and reporting them to the one owner of the strip.
//
// **A desktop's wheel is part of the pointer stream, not a second input with a scroller of its own.**
// Put the wheel in a `ScrollView` while touch goes to the grid's own handler and you have rebuilt the
// four-handlers-one-finger arrangement docs/calendar.md §6 is written from, in a costume that does
// not look like it. There is no scroller anywhere near this grid, and there must never be one.
//
// What Apple gives here that Windows does not is **phase**. A trackpad says when a gesture began,
// when the fingers lifted, and when its momentum ran out, so the strip lands on a day because the
// user let go, not because the app guessed from a moment's silence. That guess is what rubber-banded
// the Windows grid home six times in thirteen seconds on a slow pan. Only a legacy mouse wheel, which
// reports no phase at all, still needs an idle window here.

import SwiftUI

#if os(macOS)
    import AppKit
#else
    import UIKit
#endif

/// Points to move per line for a device that reports in lines rather than pixels, a classic mouse
/// wheel. A trackpad and a Magic Mouse send precise (pixel) deltas and skip this entirely.
private let linesToPoints: CGFloat = 16

/// How long a notch from a device with no phase takes to arrive, so the staircase reads as travel.
///
/// A notch that is applied the instant it lands teleports the strip once per notch and leaves it
/// perfectly still in between: a 6.5 Hz staircase, which reads as "the scroll stops at random
/// points". Easing each notch is what makes a mouse's sparse notches and a trackpad's dense stream
/// the same one continuous motion.
private let wheelEase: Double = 0.16

/// How long a phase-less device (a mouse wheel) must be silent before its gesture counts as over.
///
/// It has to clear the gap between two notches of the same gesture, or it resolves a gesture that
/// has not finished. A mouse's measured gap is about 150 ms.
private let wheelIdle: Double = 0.25

/// How long to wait after the fingers lift for momentum to declare itself. Momentum cancels it.
private let liftGrace: Double = 0.05

/// Reports wheel and trackpad scrolling over the grid. The strip's owner decides what it means.
struct CalendarScrollGesture: ViewModifier {
    /// A scroll moved the content this far, on each axis.
    let onScroll: (CGFloat, CGFloat) -> Void
    /// The gesture ended, with this much momentum left to spend. The day axis lands from there.
    let onScrollEnded: (CGFloat, CGFloat) -> Void

    func body(content: Content) -> some View {
        content.overlay(ScrollCatcher(onScroll: onScroll, onScrollEnded: onScrollEnded))
    }
}

#if os(macOS)
    /// A transparent AppKit view that reports scroll deltas over the grid.
    ///
    /// Like `PinchCatcher`, it watches through a local monitor instead of claiming the events, so it
    /// never has to become a hit-test target and costs the content below nothing.
    private struct ScrollCatcher: NSViewRepresentable {
        let onScroll: (CGFloat, CGFloat) -> Void
        let onScrollEnded: (CGFloat, CGFloat) -> Void

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

        /// Main-actor bound, and the monitor below assumes that isolation, for the reason
        /// `PinchCatcher.Coordinator` states.
        @MainActor
        final class Coordinator {
            var parent: ScrollCatcher
            private weak var view: NSView?
            private var monitor: Any?
            private var landing: Task<Void, Never>?

            init(_ parent: ScrollCatcher) { self.parent = parent }

            func watch(_ view: NSView) {
                self.view = view
                monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                    // Consumed only when it was ours; anything else travels on untouched, so the
                    // mailbox list and the sidebar keep scrolling normally. The verdict, not the
                    // event, leaves the isolated region: `NSEvent` is not `Sendable`.
                    let handled = MainActor.assumeIsolated { self?.handle(event) ?? false }
                    return handled ? nil : event
                }
            }

            func stop() {
                landing?.cancel()
                if let monitor { NSEvent.removeMonitor(monitor) }
                monitor = nil
            }

            /// Whether this scroll was over the grid, and was handled.
            ///
            /// AppKit's deltas are "how far the content should move", which is what the strip and the
            /// hour offset want, and macOS has already applied the user's natural-scrolling
            /// preference to the sign, so it is never second-guessed here.
            private func handle(_ event: NSEvent) -> Bool {
                // The monitor sees every scroll in the app, so the grid takes only the ones aimed at
                // it: same window (a sheet is a different one), pointer over the grid.
                guard let view, let window = view.window, event.window === window else { return false }
                let point = view.convert(event.locationInWindow, from: nil)
                guard view.bounds.contains(point) else { return false }

                let precise = event.hasPreciseScrollingDeltas
                let scale = precise ? 1 : linesToPoints
                var deltaX = event.scrollingDeltaX * scale
                var deltaY = event.scrollingDeltaY * scale

                // Shift+wheel pans the days, the macOS convention, and the only way a plain mouse
                // (no horizontal axis to report) can reach the rest of the strip. When AppKit has
                // already swapped the axes for this device, `deltaX` is non-zero and this leaves it
                // alone rather than swapping twice.
                if event.modifierFlags.contains(.shift), deltaX == 0 {
                    deltaX = deltaY
                    deltaY = 0
                }

                if !event.phase.isEmpty || !event.momentumPhase.isEmpty {
                    // A fresh contact, or its momentum: whatever landing was pending is stale.
                    landing?.cancel()
                }
                let moved = deltaX != 0 || deltaY != 0
                if moved {
                    if precise {
                        parent.onScroll(deltaX, deltaY)
                    } else {
                        withAnimation(.easeOut(duration: wheelEase)) {
                            parent.onScroll(deltaX, deltaY)
                        }
                    }
                }

                if event.momentumPhase.contains(.ended) || event.momentumPhase.contains(.cancelled) {
                    land(after: 0)
                } else if event.phase.contains(.ended) || event.phase.contains(.cancelled) {
                    // Momentum usually follows within a frame or two, and cancels this.
                    land(after: liftGrace)
                } else if event.phase.isEmpty, event.momentumPhase.isEmpty {
                    land(after: wheelIdle)
                }
                return moved
            }

            /// Lands the strip on a day once the gesture is over. Nothing is left to spend: a
            /// trackpad's momentum has already been applied, notch by notch, above.
            private func land(after seconds: Double) {
                landing?.cancel()
                landing = Task { @MainActor [weak self] in
                    if seconds > 0 {
                        try? await Task.sleep(for: .seconds(seconds))
                        if Task.isCancelled { return }
                    }
                    self?.parent.onScrollEnded(0, 0)
                }
            }
        }
    }
#else
    /// How much further a scroll's own momentum would carry it, as a fraction of its parting
    /// velocity: UIKit's `.normal` deceleration time constant.
    private let scrollDecay: CGFloat = 0.325

    /// A transparent UIKit view that reports an **indirect** pointer's scrolling over the grid: an
    /// iPad trackpad or a mouse. A finger never reaches this.
    ///
    /// The recognizer lives on the window rather than on this view, and that is not a shortcut. A
    /// SwiftUI overlay is a sibling of the content, not its ancestor, so a recognizer attached here
    /// would only ever fire if the view became the hit target, which would cost the grid below every
    /// tap it has. Attaching it to the window and filtering by location is the same arrangement the
    /// macOS half uses for exactly the same reason.
    private struct ScrollCatcher: UIViewRepresentable {
        let onScroll: (CGFloat, CGFloat) -> Void
        let onScrollEnded: (CGFloat, CGFloat) -> Void

        func makeCoordinator() -> Coordinator { Coordinator(self) }

        func makeUIView(context: Context) -> UIView {
            let view = PassthroughView()
            view.onAttach = { [coordinator = context.coordinator] host, window in
                coordinator.attach(over: host, to: window)
            }
            return view
        }

        func updateUIView(_ uiView: UIView, context: Context) {
            context.coordinator.parent = self
        }

        static func dismantleUIView(_ uiView: UIView, coordinator: Coordinator) {
            coordinator.detach()
        }

        /// Transparent to touches, so taps and drags fall straight through to the grid.
        final class PassthroughView: UIView {
            var onAttach: ((UIView, UIWindow) -> Void)?

            override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }

            override func didMoveToWindow() {
                super.didMoveToWindow()
                if let window { onAttach?(self, window) }
            }
        }

        @MainActor
        final class Coordinator: NSObject, UIGestureRecognizerDelegate {
            var parent: ScrollCatcher
            private weak var host: UIView?
            private weak var recognizer: UIPanGestureRecognizer?
            private var panned: CGPoint = .zero

            init(_ parent: ScrollCatcher) { self.parent = parent }

            func attach(over host: UIView, to window: UIWindow) {
                self.host = host
                if let recognizer {
                    // Already attached, and to this window: nothing to do. To a *different* one (the
                    // view moved between scenes) it has to follow, or the grid silently stops
                    // reading the trackpad.
                    guard recognizer.view !== window else { return }
                    recognizer.view?.removeGestureRecognizer(recognizer)
                }
                let pan = UIPanGestureRecognizer(target: self, action: #selector(handle(_:)))
                // Indirect input only. A finger's pan belongs to the grid's own gesture, and two
                // recognizers reading the same finger is the arrangement §6 exists to forbid.
                pan.allowedTouchTypes = []
                pan.allowedScrollTypesMask = .all
                pan.delegate = self
                window.addGestureRecognizer(pan)
                recognizer = pan
            }

            func detach() {
                guard let recognizer else { return }
                recognizer.view?.removeGestureRecognizer(recognizer)
            }

            /// Only scrolls aimed at the grid. Everything else stays with whatever owns it, so the
            /// mailbox list and the sidebar keep scrolling normally.
            func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
                guard let host, host.window != nil else { return false }
                return host.bounds.contains(gestureRecognizer.location(in: host))
            }

            func gestureRecognizer(
                _ gestureRecognizer: UIGestureRecognizer,
                shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
            ) -> Bool { true }

            @objc func handle(_ recognizer: UIPanGestureRecognizer) {
                guard let host else { return }
                switch recognizer.state {
                case .began:
                    panned = .zero
                case .changed:
                    let travel = recognizer.translation(in: host)
                    parent.onScroll(travel.x - panned.x, travel.y - panned.y)
                    panned = travel
                case .ended, .cancelled, .failed:
                    let velocity = recognizer.velocity(in: host)
                    panned = .zero
                    parent.onScrollEnded(velocity.x * scrollDecay, velocity.y * scrollDecay)
                default:
                    break
                }
            }
        }
    }
#endif
