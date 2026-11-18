// The notice row sits under the navigation bar it shares the screen with, never over it.
//
// On iPhone an inset outside a tab's navigation stack does not move that stack's bar down, so a
// row on the app's root draws over the bar and its buttons (`Mailcal.Banners.swift`). An upright
// iPad, its sidebar hidden, draws the same row. The offline notice stands for every notice, being
// the one a launch hook can raise (docs/notices.md).
//
// It also leaves the mailbox's large title drawn (`bannerBackground`).
//
// ⚠️ Measured by frames and pixels: the accessibility tree reports a button under the banner as on
// screen, and a title the banner hides as there.

import XCTest

@MainActor
final class BannerPlacementTests: XCTestCase {
    func testTheBannerSitsBelowTheBarsButtons() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_OFFLINE": "1"])
        try ShowcaseApp.showMailbox(app)
        let banner = try app.wait(.id("notice"), "the offline notice did not come up")
        XCTAssertTrue(banner.label.contains("offline"), "the notice is not the offline one: \(banner.label)")
        try assertBelow(banner, [ShowcaseApp.folders, "New Mail"], in: app)
        // The large title stays drawn. Read off the screen, because the accessibility tree reports
        // the title present whether it is drawn or not.
        let title = try app.wait(.text("Inbox").role("AXHeading"), "the mailbox has no large title")
        XCTAssertTrue(
            drawsText(in: title.frame ?? .zero), "the banner hides the mailbox's large title"
        )

        try ShowcaseApp.tab("Calendar", in: app)
        try app.wait(.button("New Event"), "the calendar did not come up")
        try assertBelow(try app.wait(.id("notice")), ["New Event", "More actions"], in: app)
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
        _ banner: Element, _ buttons: [String], in app: App,
        file: StaticString = #filePath, line: UInt = #line
    ) throws {
        let top = try XCTUnwrap(banner.frame, "the banner has no frame", file: file, line: line)
        for name in buttons {
            let button = try app.query(.button(name)).first
            let frame = try XCTUnwrap(button?.frame, "no \(name) on the bar", file: file, line: line)
            XCTAssertGreaterThanOrEqual(
                top.minY, frame.maxY, "the banner is drawn over \(name)", file: file, line: line
            )
        }
    }
}
