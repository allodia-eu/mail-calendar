// A phone composer's header follows the app's appearance (`docs/composer-layout.md`, rule 5).
//
// The header is native and drawn over the top of the editor's page, which leaves that strip
// transparent so the app's own background shows there. When the page painted its light message
// background under it, a dark app drew the header's light labels on white, and nothing below the
// UI can see a colour on screen. So this reads the pixels behind the Subject field.
//
// The appearance is set with the `MAILCAL_APPEARANCE` launch hook rather than the simulator's own,
// so the result does not depend on how the simulator was left.

import UIKit
import XCTest

@MainActor
final class ComposerAppearanceTests: XCTestCase {
    func testTheHeaderIsDarkInADarkApp() {
        let brightness = headerBrightness(appearance: "dark")
        XCTAssertLessThan(brightness, 0.3, "the header is drawn on a light background in a dark app")
    }

    func testTheHeaderIsLightInALightApp() {
        let brightness = headerBrightness(appearance: "light")
        XCTAssertGreaterThan(brightness, 0.7, "the header is drawn on a dark background in a light app")
    }

    /// How bright the background is behind the Subject field, from 0 (black) to 1 (white): read
    /// at the field's trailing end, where a placeholder never reaches.
    private func headerBrightness(appearance: String) -> CGFloat {
        let app = ShowcaseApp.launch(["MAILCAL_APPEARANCE": appearance])
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        let subject = app.textFields["Subject"]
        XCTAssertTrue(subject.waitForExistence(timeout: ShowcaseApp.timeout), "no Subject field")
        // The page applies its touch layout once it has loaded; give it that long.
        XCTAssertTrue(
            app.webViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the composer drew no editor"
        )
        Thread.sleep(forTimeInterval: 2)
        let point = CGPoint(x: subject.frame.maxX - 12, y: subject.frame.midY)
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
