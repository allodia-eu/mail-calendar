// Attach on a phone reaches the photo library (`PlatformPhotoPicker.swift`): the document picker
// lists Files' locations only, and a photograph is in none of them.
//
// Drives the system photo picker, which runs out of process, so this needs a simulator whose
// library holds a picture; every iOS simulator ships with a few. It skips, saying so, when the
// library is empty.

import XCTest

@MainActor
final class ComposerPhotoAttachTests: XCTestCase {
    func testAPhotographFromTheLibraryBecomesAFileTile() throws {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        XCTAssertTrue(app.buttons["Send"].waitForExistence(timeout: ShowcaseApp.timeout))

        ShowcaseApp.tap(app.buttons["Attach files"])
        let library = app.buttons["Photo Library"]
        XCTAssertTrue(
            library.waitForExistence(timeout: ShowcaseApp.timeout), "Attach offers no Photo Library"
        )
        XCTAssertTrue(app.buttons["Files"].exists, "Attach no longer offers Files")
        ShowcaseApp.tap(library)

        // The picker's grid, whose cells name themselves "Photo, <date>".
        let photo = app.images.matching(NSPredicate(format: "label BEGINSWITH %@", "Photo, "))
            .firstMatch
        guard photo.waitForExistence(timeout: ShowcaseApp.timeout) else {
            throw XCTSkip("the simulator's photo library shows no photograph")
        }
        photo.tap()
        // Several may be chosen, so the picker waits for Done.
        let done = app.navigationBars["Photos"].buttons["Done"]
        XCTAssertTrue(done.waitForExistence(timeout: 5), "the picker has no Done")
        ShowcaseApp.tap(done)

        // One tile, under the line that counts the files, and named as a JPEG whatever the
        // library holds it as.
        let summary = app.staticTexts.matching(
            NSPredicate(format: "label BEGINSWITH %@", "1 file")
        ).firstMatch
        XCTAssertTrue(summary.waitForExistence(timeout: ShowcaseApp.timeout), "no file was attached")
        let tile = app.staticTexts.matching(
            NSPredicate(format: "label ENDSWITH[c] %@ OR label ENDSWITH[c] %@", ".jpeg", ".jpg")
        ).firstMatch
        XCTAssertTrue(tile.exists, "the attached photograph is not named as a JPEG")
    }
}
