// A Settings control draws what the core holds, and nothing of its own: a change is sent as an
// intent and the control follows when the `Surface::Settings` signal brings the stored value back.
// So a control that stops redrawing is a broken read-back, and the control snaps back to the old
// value the moment it is let go. These tests change a preference on iPhone and read the control
// twice: at once, and after leaving the category and opening it again, which builds it anew from
// the model.
//
// The showcase persists nothing, so a change made here does not reach the next launch or the next
// test.

import UIKit
import XCTest

@MainActor
final class SettingsPreferencesTests: XCTestCase {
    func testTheDefaultZoomShowsANewStopAtOnceAndOnReturn() throws {
        try XCTSkipIf(ShowcaseApp.pad, "the iPad's Settings has a sidebar, not the phone's hub")
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"]).input
        openCategory("Calendar", in: app)
        let zoom = app.pickerWheels.element(boundBy: 1)
        XCTAssertTrue(zoom.waitForExistence(timeout: ShowcaseApp.timeout))
        let target = zoom.value as? String == "8 hours" ? "16 hours" : "8 hours"

        // A wheel that does not redraw springs back to the old stop under the drag, and `adjust`
        // then fails here, as "Unable to determine the possible values for the picker wheel".
        zoom.adjust(toPickerWheelValue: target)
        expect(zoom, toRead: target, "the default zoom did not follow the change")

        reopen("Calendar", in: app)
        expect(
            app.pickerWheels.element(boundBy: 1), toRead: target,
            "the default zoom lost the change when the category was opened again"
        )
    }

    func testTheDarkBackgroundToggleShowsAChangeAtOnceAndOnReturn() throws {
        try XCTSkipIf(ShowcaseApp.pad, "the iPad's Settings has a sidebar, not the phone's hub")
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"]).input
        openCategory("Reading", in: app)
        let toggle = darkBackgroundSwitch(in: app)
        let before = toggle.value as? String
        let target = before == "1" ? "0" : "1"

        // The row's own switch: a tap on the labelled row lands on the label, which does nothing.
        toggle.switches.firstMatch.tap()
        expect(toggle, toRead: target, "the dark background toggle did not follow the change")

        reopen("Reading", in: app)
        expect(
            darkBackgroundSwitch(in: app), toRead: target,
            "the dark background toggle lost the change when the category was opened again"
        )
    }

    /// Opens one category from the iPhone hub.
    ///
    /// By the hub row's whole label, which leads with the name and carries its summary after a
    /// comma: the tab bar under the sheet has a `Calendar` button of its own, in the tree and not
    /// reachable.
    private func openCategory(_ name: String, in app: XCUIApplication) {
        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout: ShowcaseApp.timeout))
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "\(name), "))
            .firstMatch
        for _ in 0..<3 where !(row.exists && row.isHittable) { app.swipeUp() }
        tap(row)
        XCTAssertTrue(app.navigationBars[name].waitForExistence(timeout: ShowcaseApp.timeout))
    }

    /// Goes back to the hub and opens the category again.
    private func reopen(_ name: String, in app: XCUIApplication) {
        tap(app.navigationBars[name].buttons["Settings"])
        openCategory(name, in: app)
    }

    /// The labelled switch row, scrolled into view: it sits below the fold on a phone.
    private func darkBackgroundSwitch(in app: XCUIApplication) -> XCUIElement {
        let row = app.switches["Dark background when the app is dark"]
        XCTAssertTrue(row.waitForExistence(timeout: ShowcaseApp.timeout))
        for _ in 0..<3 where !row.switches.firstMatch.isHittable { app.swipeUp() }
        return row
    }

    /// Taps `element` once it is on screen and reachable: a row below the fold exists before a
    /// finger can land on it.
    private func tap(_ element: XCUIElement) {
        XCTAssertTrue(element.waitForExistence(timeout: ShowcaseApp.timeout), "never appeared: \(element)")
        let reachable = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isHittable == true"), object: element
        )
        XCTAssertEqual(
            XCTWaiter().wait(for: [reachable], timeout: ShowcaseApp.timeout), .completed,
            "never became reachable: \(element)"
        )
        element.tap()
    }

    /// Waits for `element` to read `value`. A control that does not redraw reads the old value
    /// for good, so this times out rather than passing on a read taken mid-animation.
    private func expect(_ element: XCUIElement, toRead value: String, _ message: String) {
        let reads = NSPredicate(format: "value == %@", value)
        let settled = XCTNSPredicateExpectation(predicate: reads, object: element)
        XCTAssertEqual(
            XCTWaiter().wait(for: [settled], timeout: 5), .completed,
            "\(message): reads \(element.value ?? "nothing"), expected \(value)"
        )
    }
}
