// Where the phone's sidebar drawer sits while a finger is on it, and where it lands when the finger
// lifts.
//
// Plain functions over numbers, so the test suite drives the gesture without a device: the cases
// that matter are the ones a slow synthetic drag never reaches, a short fast flick, and a drag
// that starts in the wrong direction.

import CoreGraphics

/// The panel's leading offset mid-drag: `-width` fully closed, `0` fully open.
///
/// Clamped at both ends so pulling further than open, or pushing past closed, does not detach the
/// panel from the finger and leave a gap at the edge of the screen.
func sidebarDrawerOffset(isOpen: Bool, translation: CGFloat, width: CGFloat) -> CGFloat {
    let base: CGFloat = isOpen ? 0 : -width
    return min(0, max(-width, base + translation))
}

/// How far the panel is in, `0` fully closed to `1` fully open, what the scrim and the panel's
/// shadow both follow, so the drawer arrives and leaves as one thing rather than in layers.
func sidebarDrawerProgress(offset: CGFloat, width: CGFloat) -> CGFloat {
    guard width > 0 else { return 0 }
    return 1 + offset / width
}

/// The opacity of the panel's drop shadow at `progress`.
///
/// **Zero at zero, and that is the point.** The shadow is cast to the *trailing* side, so a panel
/// parked at `-width`, entirely off-screen, still bled a soft dark band down the leading edge of
/// the mailbox, on every screen, forever. It read as a rendering artefact because that is what it
/// was: the drawer is shut, and nothing of a shut drawer belongs on screen.
///
/// Scaled rather than switched off, so the shadow fades in with the drag exactly as the scrim does.
/// A hard `if progress > 0` would pop it in at full strength on the first frame of a swipe.
func sidebarDrawerShadowOpacity(progress: CGFloat) -> Double {
    Double(min(1, max(0, progress))) * 0.18
}

/// Whether the drawer settles open once the finger lifts.
///
/// Decided on the **predicted** end of the gesture, not where the finger actually stopped, so a
/// short fast flick opens the drawer. Judging by distance alone means a flick that travelled 40 pt
/// closes again, which reads as the drawer refusing to open.
func sidebarDrawerSettlesOpen(
    isOpen: Bool,
    predictedTranslation: CGFloat,
    width: CGFloat
) -> Bool {
    sidebarDrawerOffset(isOpen: isOpen, translation: predictedTranslation, width: width) > -width / 2
}
