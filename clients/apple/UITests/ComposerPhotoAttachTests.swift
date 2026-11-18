// Attach on a phone reaches the photo library (`PlatformPhotoPicker.swift`): the document picker
// lists Files' locations only, and a photograph is in none of them.
//
// Drives the system photo picker, which runs out of process and so is out of the agent's reach:
// the picker is driven through XCUITest, the composer through the agent. It needs a simulator whose
// library holds a picture; every iOS simulator ships with a few. It skips, saying so, when the
// library is empty.

import XCTest

@MainActor
final class ComposerPhotoAttachTests: XCTestCase {
    func testAPhotographFromTheLibraryBecomesAFileTile() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        try app.wait(.button("Send"))

        try app.press(.button("Attach files"))
        let library = try app.wait(.button("Photo Library"), "Attach offers no Photo Library")
        XCTAssertTrue(try app.exists(.button("Files")), "Attach no longer offers Files")
        try app.activate(library)

        // The picker's grid, whose cells name themselves "Photo, <date>".
        let photo = app.input.images.matching(NSPredicate(format: "label BEGINSWITH %@", "Photo, "))
            .firstMatch
        guard photo.waitForExistence(timeout: ShowcaseApp.timeout) else {
            throw XCTSkip("the simulator's photo library shows no photograph")
        }
        photo.tap()
        // Several may be chosen, so the picker waits for Done.
        let done = app.input.navigationBars["Photos"].buttons["Done"]
        XCTAssertTrue(done.waitForExistence(timeout: 5), "the picker has no Done")
        done.tap()

        // One tile, under the line that counts the files, and named as a JPEG whatever the
        // library holds it as.
        try app.wait(.textLike("1 file"), "no file was attached")
        let names = try app.query(Filter().role("AXStaticText")).map(\.label)
        XCTAssertTrue(
            names.contains { $0.lowercased().hasSuffix(".jpeg") || $0.lowercased().hasSuffix(".jpg") },
            "the attached photograph is not named as a JPEG: \(names)"
        )
    }
}
