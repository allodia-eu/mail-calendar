// Which split view does a probe name?
//
// The macOS shell nests its splits, an outer sidebar | content split, with the mailbox's own
// list | reading split inside the content pane (`Mailcal.Layout.swift`). Both splits persist their
// divider under their own autosave name, and each name is set by a zero-size probe that walks *up*
// from the pane it was dropped into. So the walk deciding "innermost" rather than "outermost" is
// what keeps the two names on two different split views.
//
// Get that wrong and nothing crashes, nothing logs, and the layout looks right until you switch
// destinations: both probes would name the outer split, one key would hold two layouts, and the
// widths would fight, which is the bug the nesting was introduced to fix.

#if os(macOS)
import AppKit
import Testing

@testable import MailcalUI

@Suite struct SplitViewAutosaveTests {
    /// Builds the shape the shell actually has: an outer split whose second pane contains an inner
    /// split. Returns both splits and a probe view sitting inside a pane of the inner one.
    private func nestedSplits() -> (outer: NSSplitView, inner: NSSplitView, innerProbe: NSView) {
        let outer = NSSplitView(frame: NSRect(x: 0, y: 0, width: 1_000, height: 600))
        let sidebar = NSView(frame: .zero)
        let inner = NSSplitView(frame: .zero)
        let listPane = NSView(frame: .zero)
        let readingPane = NSView(frame: .zero)
        // SwiftUI hosts each pane's content a couple of views deep rather than parenting it to the
        // split directly, so the probe is nested inside its pane rather than being it.
        let probe = NSView(frame: .zero)
        listPane.addSubview(probe)
        inner.addArrangedSubview(listPane)
        inner.addArrangedSubview(readingPane)
        outer.addArrangedSubview(sidebar)
        outer.addArrangedSubview(inner)
        return (outer, inner, probe)
    }

    @Test func aProbeInsideTheInnerSplitNamesTheInnerSplit() {
        let (outer, inner, probe) = nestedSplits()
        let found = nearestSplitView(from: probe)
        #expect(found === inner)
        // Spelled out separately: the failure that matters is not "found nothing", it is "found the
        // one above", and `!== outer` is the assertion that says so.
        #expect(found !== outer)
    }

    @Test func aProbeInTheOuterSplitsOwnPaneNamesTheOuterSplit() {
        let outer = NSSplitView(frame: NSRect(x: 0, y: 0, width: 1_000, height: 600))
        let sidebar = NSView(frame: .zero)
        let probe = NSView(frame: .zero)
        sidebar.addSubview(probe)
        outer.addArrangedSubview(sidebar)
        outer.addArrangedSubview(NSView(frame: .zero))
        #expect(nearestSplitView(from: probe) === outer)
    }

    @Test func theTwoProbesInANestedShellNameTwoDifferentSplits() {
        // The property the persistence depends on, stated directly: two probes, two splits, so two
        // autosave names can never land on one object.
        let (outer, inner, innerProbe) = nestedSplits()
        let sidebarProbe = NSView(frame: .zero)
        outer.arrangedSubviews[0].addSubview(sidebarProbe)
        let sidebarSplit = nearestSplitView(from: sidebarProbe)
        let mailSplit = nearestSplitView(from: innerProbe)
        #expect(sidebarSplit === outer)
        #expect(mailSplit === inner)
        #expect(sidebarSplit !== mailSplit)
    }

    @Test func aViewWithNoSplitAboveItNamesNothing() {
        // The walk has to terminate on a plain hierarchy rather than run off the end, a probe
        // reaches its superview one runloop turn *after* it is made, and SwiftUI is free to host it
        // somewhere else entirely in a future release.
        let orphan = NSView(frame: .zero)
        #expect(nearestSplitView(from: orphan) == nil)
        let plainParent = NSView(frame: .zero)
        plainParent.addSubview(orphan)
        #expect(nearestSplitView(from: orphan) == nil)
    }
}
#endif
