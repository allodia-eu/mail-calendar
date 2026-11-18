// Whether anything on screen is still moving, for `settle`.
//
// The tree's lines carry no frames, so a sheet sliding in, a menu opening or a pushed screen reads
// as quiet from its first frame. A frame read then is where the element was, and a menu opened
// while the screen under it moves is closed again by SwiftUI. So `settle` also waits until Core
// Animation has nothing running that will end, and no list is still being dragged or coasting.
//
// An animation that repeats for ever (a caret's blink, a spinner) is no motion to wait out: it would
// keep the app from ever settling. A capped `settle` names what was still moving.
//
// iOS only. The Mac's `settle` compares the tree alone, as before: an AppKit window is not
// layer-backed throughout, and nothing has measured what its layers would make `settle` do.

#if DEBUG && os(iOS)
import QuartzCore
import UIKit

@MainActor
enum AgentMotion {
    static func moving() -> Bool {
        motion() != nil
    }

    /// What is still moving, in a line, or nil: for a `settle` that gave up.
    static func motion() -> String? {
        for root in roots() {
            if let found = motion(root) { return found }
        }
        return nil
    }

    private static func roots() -> [CALayer] {
        AgentPlatform.windows().compactMap { ($0 as? UIWindow)?.layer }
    }

    private static func motion(_ layer: CALayer) -> String? {
        let owner = layer.delegate.map { String(describing: type(of: $0)) }
            ?? String(describing: type(of: layer))
        if let list = layer.delegate as? UIScrollView, list.isDragging || list.isDecelerating {
            return "\(owner) scrolling"
        }
        if layer.isHidden { return nil }
        for key in layer.animationKeys() ?? [] {
            guard let animation = layer.animation(forKey: key) else { continue }
            if forever(animation) { continue }
            return "\(owner) animating \(key) (\(type(of: animation)), repeat \(animation.repeatCount), duration \(animation.duration))"
        }
        for sublayer in layer.sublayers ?? [] {
            if let found = motion(sublayer) { return found }
        }
        return nil
    }

    /// An animation that never ends. Core Animation's "for ever" is spelt three ways: an infinite
    /// repeat, the largest finite float as the repeat, which the caret's blink uses, and an
    /// infinite duration, which a system material keeps on its view while it is shown.
    static func forever(_ animation: CAAnimation) -> Bool {
        let never = CFTimeInterval(Float.greatestFiniteMagnitude)
        return animation.repeatCount >= .greatestFiniteMagnitude
            || animation.repeatDuration >= never || animation.duration >= never
    }
}
#endif
