// A phone composer's header follows the app's appearance (`docs/composer-layout.md`, rule 5).
//
// The header is native and drawn over the top of the editor's page, which leaves that strip
// transparent so the app's own background shows there. When the page painted its light message
// background under it, a dark app drew the header's light labels on white, and nothing below the
// UI can see a colour on screen. So this reads the pixels behind the Subject field.
//
// The message under it is written on the reading view's page (`docs/reading-dark-mode.md`, rule
// 10), in the composer and in the signature editor alike: dark in a dark app, since the showcase
// leaves dark messages on, light in a light one, and light in a dark app once the reader turns dark
// messages off, which is what tells this page from the editor's own dark chrome.
//
// The appearance is set with the `MAILCAL_APPEARANCE` launch hook rather than the simulator's own,
// so the result does not depend on how the simulator was left.

import UIKit
import XCTest

@MainActor
final class ComposerAppearanceTests: XCTestCase {
    func testTheHeaderIsDarkInADarkApp() throws {
        let brightness = try headerBrightness(appearance: "dark")
        XCTAssertLessThan(brightness, 0.3, "the header is drawn on a light background in a dark app")
    }

    func testTheHeaderIsLightInALightApp() throws {
        let brightness = try headerBrightness(appearance: "light")
        XCTAssertGreaterThan(brightness, 0.7, "the header is drawn on a dark background in a light app")
    }

    func testTheMessageIsWrittenOnADarkPageInADarkApp() throws {
        let brightness = try messageBrightness(appearance: "dark")
        XCTAssertLessThan(brightness, 0.3, "the message is written on a light page in a dark app")
    }

    func testTheMessageIsWrittenOnALightPageInALightApp() throws {
        let brightness = try messageBrightness(appearance: "light")
        XCTAssertGreaterThan(brightness, 0.7, "the message is written on a dark page in a light app")
    }

    func testASignatureIsWrittenOnADarkPageInADarkApp() throws {
        let app = try SettingsCategories.open("Signatures", hooks: ["MAILCAL_APPEARANCE": "dark"])
        try app.press(.button("New signature"))
        let page = try loadedEditor(app)
        let point = CGPoint(x: page.frame.midX, y: page.frame.midY)
        let brightness = Self.brightness(at: point, of: XCUIScreen.main.screenshot().image)
        XCTAssertLessThan(brightness, 0.3, "the signature is written on a light page in a dark app")
    }

    func testASignatureIsWrittenOnALightPageOnceDarkMessagesAreOff() throws {
        let app = try SettingsCategories.open("Reading", hooks: ["MAILCAL_APPEARANCE": "dark"])
        let toggle = app.input.switches["Dark background when the app is dark"]
        XCTAssertTrue(toggle.waitForExistence(timeout: ShowcaseApp.timeout), "Reading has no switch")
        for _ in 0..<3 where !toggle.switches.firstMatch.isHittable { app.input.swipeUp() }
        XCTAssertEqual(toggle.value as? String, "1", "the showcase leaves dark messages on")
        // The row's own switch: a tap on the labelled row lands on the label, which does nothing.
        toggle.switches.firstMatch.tap()
        if !ShowcaseApp.pad {
            app.input.navigationBars["Reading"].buttons["Settings"].tap()
        }
        try SettingsCategories.go(to: "Signatures", in: app)
        try app.press(.button("New signature"))
        let page = try loadedEditor(app)
        let point = CGPoint(x: page.frame.midX, y: page.frame.midY)
        let brightness = Self.brightness(at: point, of: XCUIScreen.main.screenshot().image)
        XCTAssertGreaterThan(brightness, 0.7, "a reader who keeps messages light writes on a dark page")
    }

    /// How bright the message's page is: read at its trailing end, just above the keyboard, below
    /// the header and its row of buttons and clear of the signature.
    private func messageBrightness(appearance: String) throws -> CGFloat {
        let app = try ShowcaseApp.launch(["MAILCAL_APPEARANCE": appearance])
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        let page = try loadedEditor(app)
        let point = CGPoint(x: page.frame.maxX - 12, y: page.frame.maxY - 30)
        return Self.brightness(at: point, of: XCUIScreen.main.screenshot().image)
    }

    /// The editor's web view, once its document has drawn its toolbar. The page is web content,
    /// which only XCUITest sees; until the toolbar is there the web view shows nothing of the page.
    private func loadedEditor(_ app: App) throws -> XCUIElement {
        let page = app.input.webViews.firstMatch
        XCTAssertTrue(page.waitForExistence(timeout: ShowcaseApp.timeout), "no editor was drawn")
        XCTAssertTrue(
            page.buttons.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the editor drew no toolbar"
        )
        Thread.sleep(forTimeInterval: 1)
        return page
    }

    /// How bright the background is behind the Subject field, from 0 (black) to 1 (white): read
    /// at the field's trailing end, where a placeholder never reaches.
    private func headerBrightness(appearance: String) throws -> CGFloat {
        let app = try ShowcaseApp.launch(["MAILCAL_APPEARANCE": appearance])
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        let subject = try app.wait(.text("Subject").role("AXTextField"), "no Subject field")
        // The page is web content, which only XCUITest sees. It applies its touch layout once it
        // has loaded, and nothing on screen says when that was; give it that long.
        XCTAssertTrue(
            app.input.webViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the composer drew no editor"
        )
        Thread.sleep(forTimeInterval: 2)
        let frame = try XCTUnwrap(try app.read(subject).frame, "the Subject field has no frame")
        let point = CGPoint(x: frame.maxX - 12, y: frame.midY)
        return Self.brightness(at: point, of: XCUIScreen.main.screenshot().image)
    }

    /// The mean of the red, green and blue channels of the pixel under `point`, in points.
    private static func brightness(at point: CGPoint, of image: UIImage) -> CGFloat {
        guard let cgImage = image.cgImage else { return -1 }
        let scale = CGFloat(cgImage.width) / image.size.width
        var pixel = [UInt8](repeating: 0, count: 4)
        let space = CGColorSpaceCreateDeviceRGB()
        let info = CGImageAlphaInfo.premultipliedLast.rawValue
        pixel.withUnsafeMutableBytes { bytes in
            guard let context = CGContext(
                data: bytes.baseAddress, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                space: space, bitmapInfo: info
            ) else { return }
            let x = -(point.x * scale).rounded(.down)
            let y = -(CGFloat(cgImage.height) - (point.y * scale).rounded(.down) - 1)
            context.draw(cgImage, in: CGRect(x: x, y: y, width: CGFloat(cgImage.width), height: CGFloat(cgImage.height)))
        }
        return (CGFloat(pixel[0]) + CGFloat(pixel[1]) + CGFloat(pixel[2])) / (3 * 255)
    }
}
