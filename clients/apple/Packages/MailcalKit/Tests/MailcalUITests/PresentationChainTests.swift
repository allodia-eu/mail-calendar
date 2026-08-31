// The modal-presentation chain walk. This exists because presenting from the ROOT view controller
// shipped: on iOS/iPadOS the composer is a `fullScreenCover`, so the root is already presenting,
// and UIKit refuses a second present on it *silently*, Apple's review reported it as "tapped
// 'attach file' but there was no response" and rejected the build.
//
// Deliberately written against `PresentationChainNode` rather than UIKit: `swift test` runs on
// macOS, so an `#if os(iOS)` test of this would compile to nothing and report a pass over the very
// code that broke.

import Testing

@testable import MailcalUI

/// A stand-in for one `UIViewController` in a presentation chain.
@MainActor
private final class FakeNode: PresentationChainNode {
    let name: String
    var presenting: FakeNode?
    var dismissing = false

    init(_ name: String) { self.name = name }

    var nextPresented: (any PresentationChainNode)? { presenting }
    var isDismissingNow: Bool { dismissing }
}

@MainActor struct PresentationChainTests {
    /// Nothing presented: the root is the only candidate.
    @Test func idleRootIsItsOwnTop() {
        let root = FakeNode("root")
        #expect(topOfPresentationChain(from: root) as? FakeNode === root)
    }

    /// The regression: the composer is up, so a modal must be presented from the composer, NOT the
    /// root. Returning the root here is the exact bug Apple rejected the build for.
    @Test func walksPastAPresentingRoot() {
        let root = FakeNode("root")
        let composer = FakeNode("composer")
        root.presenting = composer

        let top = topOfPresentationChain(from: root) as? FakeNode

        #expect(top === composer)
        #expect(top !== root, "presenting from a busy root is silently ignored by UIKit")
    }

    /// Chains nest more than one deep, the account sheet opens over the composer, and the file
    /// picker still has to land on top of both.
    @Test func walksToTheDeepestPresentedNode() {
        let root = FakeNode("root")
        let composer = FakeNode("composer")
        let sheet = FakeNode("sheet")
        root.presenting = composer
        composer.presenting = sheet

        #expect(topOfPresentationChain(from: root) as? FakeNode === sheet)
    }

    /// A controller on its way out cannot be presented from either, so the walk stops before it.
    @Test func stopsBeforeANodeBeingDismissed() {
        let root = FakeNode("root")
        let closing = FakeNode("closing")
        closing.dismissing = true
        root.presenting = closing

        #expect(topOfPresentationChain(from: root) as? FakeNode === root)
    }
}
