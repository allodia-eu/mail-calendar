// Launching and steering the app under test.
//
// Every test in this bundle drives the in-memory **showcase** dataset. It needs no network, no
// Docker harness and no stored account, which is what lets one suite run on a simulator and on a
// physical iPhone: the local mail harness is loopback-only and a device cannot reach it
// (`scripts/dev/device.sh`).
//
// The language is pinned with `-AppleLanguages`, the argument the screenshot runs already use
// (`scripts/dev/showcase.sh`), so a Dutch Mac and a CI runner drive the same English chrome. The
// strings asserted in the tests are written out rather than read back from the app: a test that
// asks the app what it says and then checks it says that cannot fail.

import XCTest

@MainActor
enum ShowcaseApp {
    /// How long to wait for a surface that has to load mail before it can draw.
    static let timeout: TimeInterval = 30

    /// Launches on the showcase dataset, in English.
    ///
    /// `hooks` carries the `MAILCAL_*` launch hooks (`docs/debugging.md`). Reaching a screen
    /// through one is deterministic where tapping through the UI is not, so prefer a hook wherever
    /// the app offers one.
    static func launch(_ hooks: [String: String] = [:]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(en)"]
        app.launchEnvironment = hooks.merging(["MAILCAL_SHOWCASE": "en"]) { current, _ in current }
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: timeout), "the app did not launch")
        return app
    }

    /// Brings the mailbox up, wherever the app happens to have started.
    ///
    /// The app restores the surface it was last on (`@SceneStorage`) and a test process launches it
    /// many times, so a test that assumes a starting screen is a test whose result depends on the
    /// test before it.
    ///
    /// ⚠️ The destination switcher on iPhone is the **bottom tab bar**, not the folder drawer. The
    /// drawer's dimming scrim is a full-height button of its own ("Close folders"), and it covers
    /// the tab bar while the drawer is open, so a route that opens the drawer first finds Mail
    /// present, on screen, and not reachable, and the tap that looks like it should switch
    /// destination shuts the drawer instead. Tapping the tab while already on it pops back to the
    /// list, which is where the tests want to start anyway.
    static func showMailbox(_ app: XCUIApplication) {
        tap(app.buttons["Mail"])
        XCTAssertTrue(
            app.buttons["Compose"].waitForExistence(timeout: timeout),
            "the mailbox did not come up"
        )
    }

    /// Taps an element once it is both on screen and reachable.
    ///
    /// `exists` is not enough on iPhone: the folder drawer's rows stay in the accessibility tree
    /// while it is shut, parked off the left edge, so a tap on an existing row lands on whatever is
    /// at negative coordinates, which is nothing.
    static func tap(_ element: XCUIElement) {
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "never appeared: \(element)")
        let hittable = NSPredicate(format: "isHittable == true")
        let reachable = XCTNSPredicateExpectation(predicate: hittable, object: element)
        XCTAssertEqual(
            XCTWaiter().wait(for: [reachable], timeout: timeout), .completed,
            "never became reachable: \(element)"
        )
        element.tap()
    }

    /// Whether a tap aimed at the button called `name` would land on it.
    ///
    /// ⚠️ Not `app.buttons[name].isHittable`, and what makes the difference is the iOS version
    /// rather than the layout. A SwiftUI button that presents a menu is published as a button
    /// wrapping a second button wrapping an image, **all three at the same frame**, and `isHittable`
    /// is answered by whichever of them the hit test reaches last: the outer button on iOS 26, the
    /// innermost image on iOS 18, because each child covers its parent exactly. All three are one
    /// control to a reader and to VoiceOver, so the question asked here is whether *anything* at
    /// that name's frame can be tapped.
    ///
    /// Asking the outer element alone passes on a current simulator and fails on a real phone a
    /// version or two behind, reporting a toolbar item that is on screen and works as missing. That
    /// is the shape of failure a suite which only ever runs on the newest simulator never sees, and
    /// it is what the device leg is for.
    static func reachable(_ name: String, in app: XCUIApplication) -> Bool {
        let button = app.buttons[name]
        guard button.exists else { return false }
        if button.isHittable { return true }
        // `descendants(matching:)` does not include the element itself, so a plain button that is
        // its own hit target needs the line above; only a wrapped one reaches this.
        return button.descendants(matching: .any).allElementsBoundByIndex
            .contains(where: \.isHittable)
    }

    /// The on-screen order of `labels`, left to right, as the reader meets them.
    ///
    /// Measured off the frames rather than read out of the tree: the rule under test is about what
    /// the eye meets first, and tree order is not evidence of that.
    ///
    /// Off-screen matches are dropped, and that is not tidiness. The folder drawer keeps its rows
    /// in the tree while it is shut, parked at negative coordinates, so a mailbox named Archive is
    /// a second `Archive` button on every reading screen; left in, it sorts ahead of the whole
    /// action row and the order under test reads as broken.
    static func leftToRight(_ labels: [String], in app: XCUIApplication) -> [String] {
        let screen = app.windows.firstMatch.frame
        return app.buttons
            .matching(NSPredicate(format: "label IN %@", labels))
            .allElementsBoundByIndex
            .map { (label: $0.label, frame: $0.frame) }
            .filter { screen.contains(CGPoint(x: $0.frame.midX, y: $0.frame.midY)) }
            .sorted { $0.frame.midX < $1.frame.midX }
            .map(\.label)
    }
}
