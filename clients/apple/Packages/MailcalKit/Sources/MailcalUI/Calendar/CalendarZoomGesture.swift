// The pinch gesture.
//
// SwiftUI's `MagnifyGesture` reports a single **scalar** magnification. That is enough to zoom the
// hours, and not enough for anything else: a calendar pinch is genuinely two gestures, spread your
// fingers vertically and you want more hours; spread them sideways and you want fewer days; spread
// them diagonally and you want both, each by its own component. A scalar cannot tell those apart.
//
// So both platforms reach underneath SwiftUI to where the two touches still are:
//
//   - **iOS**, `UIPinchGestureRecognizer`, which reports each touch's location in the view.
//   - **macOS**, the raw `NSEvent`, whose magnify events carry the `NSTouch` objects the trackpad
//     sent. Each one knows its position on the trackpad surface, so the per-axis spread is
//     recoverable there too, and a Mac gets the same diagonal pinch a phone does.

import SwiftUI

#if os(iOS)
    import UIKit
#else
    import AppKit
#endif

/// How far apart two fingers must be **on an axis** before that axis's scale means anything.
///
/// This is what keeps the axes independent without forbidding diagonals. Fingers spread purely
/// sideways sit at almost the same height, so their vertical spread is a few noisy points, dividing
/// by it would produce a wild factor and lurch the hours about while the user was only asking for
/// more days. Spread them at an angle and *both* spreads are real, so both axes zoom.
private let minSpread: CGFloat = 48

/// One axis's scale, or exactly `1` when the fingers are too close together on it to know.
///
/// `minimum` is a parameter rather than a constant because a **trackpad does not measure in points**:
/// it reports touches in its own device units, so the phone's floor is meaningless there and the
/// caller passes one scaled to the device.
func axisScale(before: CGFloat, after: CGFloat, minimum: CGFloat = minSpread) -> CGFloat {
    (before < minimum || after < minimum) ? 1 : after / before
}

/// Applies a pinch to the grid's zoom, anchoring the content under the fingers.
struct CalendarZoomGesture: ViewModifier {
    @Binding var zoom: CalendarZoom
    @Binding var strip: CalendarStrip
    @Binding var hourOffset: CGFloat
    /// The width the day columns scroll through: the surface, less the pinned hour ruler.
    let viewportWidth: CGFloat
    let viewportHeight: CGFloat
    let onSettled: () -> Void

    func body(content: Content) -> some View {
        content.overlay(
            PinchCatcher(
                onPinch: { xScale, yScale, focus in
                    apply(xScale: xScale, yScale: yScale, focus: focus)
                },
                onSettled: onSettled
            )
        )
    }

    /// Zooms both axes, and moves both scrolls so whatever was under the fingers stays under them.
    ///
    /// Each axis is corrected by the factor its zoom **actually applied**, not the one it was asked
    /// for: at a clamp that is 1. Correcting by the requested factor there would drag the grid on
    /// every further frame of a pinch that has nowhere left to go, and would let an exhausted hour
    /// axis drag the day axis to a halt mid-diagonal.
    private func apply(xScale: CGFloat, yScale: CGFloat, focus: CGPoint) {
        let vertical = zoom.pinchVertical(yScale)
        if vertical != 1 {
            let target = focalPreservingScroll(scroll: hourOffset, focus: focus.y, factor: vertical)
            // The content grows as it zooms, so the new bound is measured from the NEW hour height,
            // not the one the caller last laid out with.
            let newContent = zoom.hourHeight(viewport: viewportHeight) * CGFloat(calendarHours)
            hourOffset = min(max(target, 0), max(newContent - viewportHeight, 0))
        }
        // The day axis anchors in the **day viewport**, past the pinned hour ruler, which is where
        // the strip's own arithmetic begins. The fingers' point is reported over the whole grid, so
        // the gutter comes off it first; without that the columns creep sideways under a pinch by
        // the ruler's width, and no clamp is left to hide it.
        let dayWidthBefore = zoom.dayWidth(viewport: viewportWidth)
        let horizontal = zoom.pinchHorizontal(xScale)
        if horizontal != 1 {
            strip.pinch(
                factor: horizontal,
                focus: focus.x - calendarGutter,
                dayWidthBefore: dayWidthBefore,
                dayWidthAfter: zoom.dayWidth(viewport: viewportWidth)
            )
        }
    }
}

#if os(macOS)
    /// Fingers must be spread at least this fraction of the **trackpad's own size** along an axis
    /// before that axis's scale means anything.
    ///
    /// The floor is relative because a trackpad reports touches in device units, not points, so
    /// there is no fixed number of "points apart" to compare against, and a hard-coded one would mean
    /// something different on every Mac. A tenth of the surface is about what 48pt is on a phone.
    private let minTrackpadSpread: CGFloat = 0.1

    /// A transparent AppKit view that watches the raw magnify events and reports **where the fingers
    /// are on the trackpad**, which is the one thing SwiftUI's `MagnifyGesture` will not tell us.
    ///
    /// It watches via a local event monitor rather than by claiming the gesture, precisely so it does
    /// not have to become a hit-test target: the grid underneath keeps every click, drag and scroll it
    /// had, and this sees the pinch anyway.
    private struct PinchCatcher: NSViewRepresentable {
        let onPinch: (CGFloat, CGFloat, CGPoint) -> Void
        let onSettled: () -> Void

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

        /// Never a hit-test target, so it costs the content below nothing.
        ///
        /// It does **ask for indirect touches**, and that request is what makes a diagonal pinch
        /// possible at all. A magnify event carries the trackpad's `NSTouch` objects only while some
        /// view in the window has asked for them; with nobody asking, `touches(matching:)` comes back
        /// empty, the catcher falls through to the scalar `magnification` below, and a scalar can
        /// only ever zoom one axis. The symptom is a pinch that changes the hours and leaves the
        /// columns exactly as wide as they were, which reads as arithmetic rather than as a missing
        /// request.
        final class PassthroughView: NSView {
            override func hitTest(_ point: NSPoint) -> NSView? { nil }
            /// Top-left origin, to match the SwiftUI coordinates the focal point is used in.
            override var isFlipped: Bool { true }

            override func viewDidMoveToWindow() {
                super.viewDidMoveToWindow()
                allowedTouchTypes = [.indirect]
                // And on the content view, because this one is deliberately not a hit-test target:
                // the view the touches are delivered against is whatever the pointer is over, which
                // is the grid's own content. The request has to be made somewhere that is always in
                // the chain, whatever that is.
                window?.contentView?.allowedTouchTypes.insert(.indirect)
            }
        }

        /// Main-actor bound: everything it touches (the view, the event, the parent's callbacks)
        /// is, and the local monitor below is called on the main thread as part of the app's own
        /// event dispatch. AppKit's block is imported without that isolation, so `assumeIsolated`
        /// is where the fact is stated; it is an assertion, not a hop, so a wrong assumption would
        /// trap here rather than race somewhere else.
        @MainActor
        final class Coordinator {
            var parent: PinchCatcher
            private weak var view: NSView?
            private var monitor: Any?
            private var lastSpread: CGSize?
            /// Whether the gesture in progress is a **pinch**, which only a `.magnify` says.
            ///
            /// `.beginGesture`/`.endGesture` bracket every trackpad gesture, a two-finger *scroll*
            /// included, and the contacts ride on the `.gesture` stream either way. Without this
            /// flag a plain sideways scroll reads as a spread, zooms the day axis, and then settles:
            /// `onSettled` persists the horizon and the layout to the core, so scrolling the
            /// calendar would rewrite the shape the user chose.
            private var magnifying = false

            init(_ parent: PinchCatcher) { self.parent = parent }

            func watch(_ view: NSView) {
                self.view = view
                // **The touches and the magnification arrive on different events**, so both are
                // watched. A `.magnify` carries a scalar and says when the gesture began and ended;
                // the trackpad's individual contacts ride on the `.gesture` stream beside it, which
                // is the only place the per-axis spread can be read. Watching magnify alone left
                // `touches(matching:)` empty on every frame, so the catcher fell through to its
                // scalar fallback and the pinch could only ever move the hours.
                monitor = NSEvent.addLocalMonitorForEvents(
                    matching: [.magnify, .gesture, .beginGesture, .endGesture]
                ) { [weak self] event in
                    MainActor.assumeIsolated { self?.handle(event) }
                    return event
                }
            }

            func stop() {
                if let monitor { NSEvent.removeMonitor(monitor) }
                monitor = nil
            }

            private func handle(_ event: NSEvent) {
                // The monitor sees every gesture in the app, so the grid only takes the ones aimed
                // at it: same window, cursor over the grid.
                guard let view, let window = view.window, event.window === window else { return }
                let focus = view.convert(event.locationInWindow, from: nil)
                guard view.bounds.contains(focus) else { return }

                if event.type == .beginGesture {
                    lastSpread = nil
                    magnifying = false
                    return
                }

                if event.type == .endGesture || event.phase == .ended || event.phase == .cancelled {
                    lastSpread = nil
                    // Only a gesture that actually magnified has a zoom to settle.
                    if magnifying { parent.onSettled() }
                    magnifying = false
                    return
                }

                // macOS decides what a pair of fingers means, and says so with `.magnify`. Until it
                // has, the contacts on the `.gesture` stream beside it belong to a scroll.
                if event.type == .magnify { magnifying = true }
                guard magnifying else { return }

                // The contacts are the truth for **both** axes, whichever event brought them, so the
                // zoom is applied from them and `magnification` is needed only when they are absent.
                if let spread = axisSpread(of: event.allTouches()) {
                    defer { lastSpread = spread.gap }
                    guard let previous = lastSpread else { return }
                    let xScale = axisScale(
                        before: previous.width, after: spread.gap.width,
                        minimum: spread.device.width * minTrackpadSpread
                    )
                    let yScale = axisScale(
                        before: previous.height, after: spread.gap.height,
                        minimum: spread.device.height * minTrackpadSpread
                    )
                    if xScale != 1 || yScale != 1 {
                        parent.onPinch(xScale, yScale, focus)
                    }
                    return
                }

                // Something is magnifying that is not two fingers on a trackpad: a Magic Mouse, a
                // tablet driver. There are no axes to read, so fall back to what a scalar can still
                // say: zoom the hours, which is the axis a desktop user reaches for anyway.
                guard event.type == .magnify else { return }
                let scale = 1 + event.magnification
                if scale > 0 { parent.onPinch(1, scale, focus) }
            }

            /// How far apart the two fingers are on each axis, in the trackpad's own units, and how
            /// big the trackpad is, which is the only thing that gives those units a meaning.
            private func axisSpread(of touches: Set<NSTouch>) -> (gap: CGSize, device: CGSize)? {
                let pair = Array(touches.filter { $0.phase != .ended && $0.phase != .cancelled })
                guard pair.count == 2 else { return nil }
                let device = pair[0].deviceSize
                guard device.width > 0, device.height > 0 else { return nil }
                let a = pair[0].normalizedPosition
                let b = pair[1].normalizedPosition
                let gap = CGSize(
                    width: abs(a.x - b.x) * device.width,
                    height: abs(a.y - b.y) * device.height
                )
                return (gap, device)
            }
        }
    }
#endif

#if os(iOS)
    /// A transparent UIKit view whose only job is to own a two-finger pinch and report **where the
    /// fingers are**, which is the one thing SwiftUI's gesture will not tell us.
    private struct PinchCatcher: UIViewRepresentable {
        let onPinch: (CGFloat, CGFloat, CGPoint) -> Void
        let onSettled: () -> Void

        func makeCoordinator() -> Coordinator { Coordinator(self) }

        func makeUIView(context: Context) -> UIView {
            let view = PassthroughView()
            let pinch = UIPinchGestureRecognizer(
                target: context.coordinator, action: #selector(Coordinator.handle(_:))
            )
            // Let the scroll views underneath keep working: a one-finger drag is theirs, and a pinch
            // only ever involves two.
            pinch.delegate = context.coordinator
            view.addGestureRecognizer(pinch)
            return view
        }

        func updateUIView(_ uiView: UIView, context: Context) {
            context.coordinator.parent = self
        }

        /// Transparent to touches it does not itself claim, so the grid below stays scrollable.
        final class PassthroughView: UIView {
            override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
                // Never become the hit target: the gesture recognizer still sees every touch, but
                // taps and drags fall straight through to the content.
                nil
            }
        }

        final class Coordinator: NSObject, UIGestureRecognizerDelegate {
            var parent: PinchCatcher
            private var lastSpread: CGSize?

            init(_ parent: PinchCatcher) { self.parent = parent }

            func gestureRecognizer(
                _ gestureRecognizer: UIGestureRecognizer,
                shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
            ) -> Bool { true }

            @objc func handle(_ recognizer: UIPinchGestureRecognizer) {
                guard let view = recognizer.view, recognizer.numberOfTouches == 2 else {
                    if recognizer.state == .ended || recognizer.state == .cancelled {
                        lastSpread = nil
                        parent.onSettled()
                    }
                    return
                }
                let a = recognizer.location(ofTouch: 0, in: view)
                let b = recognizer.location(ofTouch: 1, in: view)
                let spread = CGSize(width: abs(a.x - b.x), height: abs(a.y - b.y))

                switch recognizer.state {
                case .began:
                    lastSpread = spread
                case .changed:
                    guard let previous = lastSpread else {
                        lastSpread = spread
                        return
                    }
                    let xScale = axisScale(before: previous.width, after: spread.width)
                    let yScale = axisScale(before: previous.height, after: spread.height)
                    lastSpread = spread
                    if xScale != 1 || yScale != 1 {
                        let focus = CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2)
                        parent.onPinch(xScale, yScale, focus)
                    }
                case .ended, .cancelled, .failed:
                    lastSpread = nil
                    parent.onSettled()
                default:
                    break
                }
            }
        }
    }
#endif
