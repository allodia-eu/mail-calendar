// The touch composer's header fits a phone: the From label keeps its width, and the action bar is
// symbols in one row with the draft's actions behind More (`RichComposerView.ActionBar.swift`).
//
// Run on a phone with two accounts and a signature, which is what the showcase is: the sender
// picker and the signature menu both appear only then, and with them a header too wide for the
// screen squeezed "From" to a letter a line and wrapped every button's title.

import XCTest

@MainActor
final class ComposerActionBarTests: XCTestCase {
    func testTheHeaderFitsAPhoneAndKeepsEveryAction() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        try app.wait(.button("Send"), "Compose did not open the composer")

        let from = try XCTUnwrap(
            try app.wait(.text("From").role("AXStaticText")).frame, "From has no frame"
        )
        XCTAssertGreaterThan(from.width, from.height, "From is squeezed into a column")

        let screen = app.input.windows.firstMatch.frame
        for name in ["Attach files", "Signature", "More actions"] {
            let button = try app.query(.button(name)).first
            let frame = try XCTUnwrap(button?.frame, "the action bar has no \(name)")
            XCTAssertTrue(screen.contains(frame), "\(name) runs off the screen")
            XCTAssertLessThanOrEqual(frame.height, 50, "\(name) wraps onto more lines")
        }

        try app.press(.button("More actions"))
        try app.wait(.button("Save as draft"), "More holds no Save as draft")
        try app.press(.button("Discard"))
        // An untouched message closes at once; one with something to lose asks first.
        if try !app.goes(.button("Send"), timeout: 2) {
            try app.press(.button("Discard"))
        }
        try app.wait(.button("New Mail"), "Discard did not close the composer")
    }
}
