// A banner sits under the navigation bar it shares the screen with, never over it.
//
// On iPhone an inset outside a tab's navigation stack does not move that stack's bar down, so a
// banner on the app's root draws over the bar and its buttons (`Mailcal.Banners.swift`). The
// offline banner stands for all of them, being the one a launch hook can raise.
//
// It also leaves the mailbox's large title drawn (`bannerBackground`).
//
// ⚠️ Measured by frames and pixels, not by `isHittable`: XCUITest answers that a button under the
// banner is hittable, and that a title the banner hides is there.

import XCTest

@MainActor
final class BannerPlacementTests: XCTestCase {
    func testTheBannerSitsBelowTheBarsButtons() {
        let app = ShowcaseApp.launch(["MAILCAL_OFFLINE": "1"])
        ShowcaseApp.showMailbox(app)
        let banner = app.staticTexts["You're offline, showing saved mail."]
        XCTAssertTrue(
            banner.waitForExistence(timeout: ShowcaseApp.timeout),
            "the offline banner did not come up"
        )
        assertBelow(banner, ["Folders", "New Mail"], in: app)
        // The large title stays drawn. Read off the screen, because the accessibility tree reports
        // the title present and hittable whether it is drawn or not.
        let title = app.navigationBars.staticTexts["Inbox"]
        XCTAssertTrue(title.exists, "the mailbox has no large title")
        XCTAssertTrue(drawsText(in: title.frame), "the banner hides the mailbox's large title")

        ShowcaseApp.tap(app.buttons["Calendar"])
        XCTAssertTrue(
            app.buttons["New Event"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar did not come up"
        )
        assertBelow(banner, ["New Event", "More actions"], in: app)
    }

    /// Whether the screen shows anything inside `frame` (in points): text spans a wide range of
    /// brightness, an empty band one flat fill.
    private func drawsText(in frame: CGRect) -> Bool {
        let shot = XCUIScreen.main.screenshot().image
        guard let image = shot.cgImage else { return false }
        let scale = shot.scale
        let pixels = CGRect(
            x: frame.minX * scale, y: frame.minY * scale,
            width: frame.width * scale, height: frame.height * scale
        ).integral
        guard let crop = image.cropping(to: pixels) else { return false }
        let width = crop.width, height = crop.height
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        let drawn = bytes.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            context.draw(crop, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return false }
        let luminance = stride(from: 0, to: bytes.count, by: 4).map {
            (Int(bytes[$0]) * 3 + Int(bytes[$0 + 1]) * 6 + Int(bytes[$0 + 2])) / 10
        }
        return (luminance.max() ?? 0) - (luminance.min() ?? 0) > 96
    }

    private func assertBelow(
        _ banner: XCUIElement, _ buttons: [String], in app: XCUIApplication,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        for name in buttons {
            let button = app.buttons[name]
            XCTAssertTrue(button.exists, "no \(name) on the bar", file: file, line: line)
            XCTAssertGreaterThanOrEqual(
                banner.frame.minY, button.frame.maxY,
                "the banner is drawn over \(name)", file: file, line: line
            )
        }
    }
}
