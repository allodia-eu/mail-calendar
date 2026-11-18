// A photograph shared from the Photos app arrives in a composer (`ShareExtension/`,
// `docs/os-integration.md`).
//
// Drives Photos and the system share sheet, both other apps, so it needs a simulator whose library
// holds a picture (every iOS simulator ships with a few) and skips, saying so, when it cannot reach
// one. The app is launched first, on the showcase, because a share opens a composer only for an
// app with an account to send from.

import XCTest

@MainActor
final class SharePhotoTests: XCTestCase {
    func testAPhotographSharedFromPhotosOpensAComposerHoldingIt() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)

        let photos = XCUIApplication(bundleIdentifier: "com.apple.mobileslideshow")
        photos.launch()
        // A first launch shows what is new before the library.
        let carryOn = photos.buttons["Continue"]
        if carryOn.waitForExistence(timeout: 3) { carryOn.tap() }

        // Photos reopens where it was left, which may already be one photograph.
        let share = photos.buttons["PUOneUpBarButtonItemIdentifierShare"]
        if !share.waitForExistence(timeout: 3) {
            // A grid cell names itself "Photo, <date>"; the Library tab's own glyph starts "Photo"
            // as well.
            let photo = photos.images.matching(NSPredicate(format: "label BEGINSWITH %@", "Photo, "))
                .firstMatch
            guard photo.waitForExistence(timeout: ShowcaseApp.timeout) else {
                throw XCTSkip("the simulator's Photos app shows no photograph")
            }
            photo.tap()
        }
        XCTAssertTrue(share.waitForExistence(timeout: ShowcaseApp.timeout), "Photos offers no Share")
        share.tap()

        // The share sheet labels the extension with the app's own name, which the brand decides.
        let name = app.input.label
        let target = shareSheetTarget(named: name, in: photos)
        guard target.waitForExistence(timeout: ShowcaseApp.timeout) else {
            throw XCTSkip("the share sheet does not list \(name)")
        }
        target.tap()

        // The extension stages the photograph and brings the app forward itself: a share sheet
        // that only closes reads as a share that went nowhere.
        XCTAssertTrue(
            app.input.wait(for: .runningForeground, timeout: ShowcaseApp.timeout),
            "the share did not bring the app forward"
        )
        try app.wait(.button("Send"), "no composer opened for the shared photograph")
        try app.wait(.textLike("1 file"), "the composer opened without the photograph")
        XCTAssertFalse(
            try app.exists(.textLike("Couldn't attach")),
            "the composer says the photograph could not be attached"
        )
    }

    /// The app's entry in the share sheet: in the row of apps, or behind More when the row is full.
    private func shareSheetTarget(named name: String, in photos: XCUIApplication) -> XCUIElement {
        let inRow = photos.cells[name]
        if inRow.waitForExistence(timeout: 5) { return inRow }
        let more = photos.cells["More"]
        if more.exists {
            more.tap()
            return photos.cells.staticTexts[name]
        }
        return inRow
    }
}
