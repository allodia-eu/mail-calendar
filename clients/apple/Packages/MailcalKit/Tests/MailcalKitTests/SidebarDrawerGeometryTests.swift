// The phone sidebar drawer's gesture arithmetic (SidebarDrawerGeometry.swift).

import CoreGraphics
import Testing

@testable import MailcalUI

@Suite struct SidebarDrawerGeometryTests {
    private let width: CGFloat = 320

    @Test func aClosedDrawerFollowsTheFingerInFromTheEdge() {
        #expect(sidebarDrawerOffset(isOpen: false, translation: 0, width: width) == -320)
        #expect(sidebarDrawerOffset(isOpen: false, translation: 100, width: width) == -220)
        #expect(sidebarDrawerOffset(isOpen: false, translation: 320, width: width) == 0)
    }

    @Test func anOpenDrawerFollowsTheFingerBackOut() {
        #expect(sidebarDrawerOffset(isOpen: true, translation: 0, width: width) == 0)
        #expect(sidebarDrawerOffset(isOpen: true, translation: -120, width: width) == -120)
    }

    @Test func thePanelNeverDetachesFromEitherEdge() {
        // Dragging past open must not push the panel off the trailing side, and pushing past closed
        // must not open a gap at the leading edge, both leave the screen visibly broken.
        #expect(sidebarDrawerOffset(isOpen: true, translation: 500, width: width) == 0)
        #expect(sidebarDrawerOffset(isOpen: false, translation: -500, width: width) == -320)
        #expect(sidebarDrawerOffset(isOpen: true, translation: -500, width: width) == -320)
    }

    @Test func aShortFastFlickOpensIt() {
        // The case a slow drag never reaches, and the one a distance-only rule gets wrong: the
        // finger stopped 40 pt in, but it was still moving, so the gesture was going to finish open.
        #expect(sidebarDrawerSettlesOpen(isOpen: false, predictedTranslation: 300, width: width))
        // The same 40 pt with no momentum behind it falls back shut.
        #expect(!sidebarDrawerSettlesOpen(isOpen: false, predictedTranslation: 40, width: width))
    }

    @Test func lettingGoPastTheHalfwayMarkOpensIt() {
        #expect(sidebarDrawerSettlesOpen(isOpen: false, predictedTranslation: 161, width: width))
        #expect(!sidebarDrawerSettlesOpen(isOpen: false, predictedTranslation: 159, width: width))
    }

    @Test func anOpenDrawerNeedsHalfAWidthOfIntentToShut() {
        // A stray horizontal nudge while scrolling the folder list must not dismiss it.
        #expect(sidebarDrawerSettlesOpen(isOpen: true, predictedTranslation: -20, width: width))
        #expect(!sidebarDrawerSettlesOpen(isOpen: true, predictedTranslation: -200, width: width))
    }

    @Test func aShutDrawerCastsNoShadow() {
        // The regression, and it shipped: the shadow is cast to the TRAILING side, so a panel
        // parked off-screen at -width still bled a dark band down the leading edge of the mailbox
        // on every screen, in every language, including the store screenshots.
        let shut = sidebarDrawerOffset(isOpen: false, translation: 0, width: width)
        #expect(sidebarDrawerShadowOpacity(
            progress: sidebarDrawerProgress(offset: shut, width: width)
        ) == 0)
    }

    @Test func theShadowArrivesWithTheDragRatherThanPoppingIn() {
        // Scaled, not switched: at the halfway point it is halfway up. A `progress > 0` test would
        // put the full shadow on screen on the first frame of a swipe.
        let half = sidebarDrawerOffset(isOpen: false, translation: 160, width: width)
        let progress = sidebarDrawerProgress(offset: half, width: width)
        #expect(progress == 0.5)
        #expect(sidebarDrawerShadowOpacity(progress: progress) == 0.09)
        #expect(sidebarDrawerShadowOpacity(progress: 1) == 0.18)
    }

    @Test func aDegenerateWidthIsShutRatherThanDividedBy() {
        // `width` is derived from the screen minus an inset, so it is only ever ~0 on a container
        // that cannot show the drawer at all. Answering "shut" keeps that case off screen; dividing
        // would put a NaN into an opacity and take the whole layer with it.
        #expect(sidebarDrawerProgress(offset: 0, width: 0) == 0)
        #expect(sidebarDrawerShadowOpacity(progress: sidebarDrawerProgress(offset: 0, width: 0)) == 0)
    }
}
