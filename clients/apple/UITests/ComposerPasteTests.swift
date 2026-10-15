// A list pasted from Word is a list on iPhone and iPad (docs/composer-security.md, Gate 7).
//
// iOS WebKit rebuilds another app's HTML through an attributed string before the page sees it, which
// drops the `mso-list` style Word marks its list paragraphs with, so the bundle alone would paste
// each item as a line starting with "·". The composer's web view reads the clipboard's HTML itself
// (`EditorWebViewTouch.swift`). Only this suite can see that: the bundle's own tests never meet
// WebKit, and the rewrite happens on the way into the page.

import UIKit
import XCTest

@MainActor
final class ComposerPasteTests: XCTestCase {
    func testAListPastedFromWordIsAList() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        let web = app.webViews.firstMatch
        XCTAssertTrue(web.waitForExistence(timeout: ShowcaseApp.timeout), "the composer drew no editor")

        UIPasteboard.general.setItems([[
            "public.html": Data(Self.wordList.utf8),
            "public.utf8-plain-text": "Apples\nPears",
        ]])
        let body = web.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.55))
        body.tap()
        choose("Paste", in: app, at: body)

        let apples = web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "Apples"))
        XCTAssertTrue(apples.firstMatch.waitForExistence(timeout: 5), "nothing was pasted")
        // At the start of a text: the showcase signature reads "Product lead · Northwind".
        let typedBullet = NSPredicate(format: "label BEGINSWITH %@", "\u{00B7}")
        XCTAssertEqual(
            web.staticTexts.matching(typedBullet).count, 0,
            "Word's bullet arrived as text, so the list did not:\n\(web.debugDescription)"
        )
    }

    /// Taps an item of the edit menu, raising the menu at `point` until it shows.
    private func choose(_ item: String, in app: XCUIApplication, at point: XCUICoordinate) {
        for _ in 0..<4 {
            if app.menuItems[item].waitForExistence(timeout: 2) {
                app.menuItems[item].tap()
                return
            }
            point.tap()
        }
        XCTFail("the edit menu offered no \(item)")
    }

    /// Word for Mac (Microsoft 365) copying a bulleted list of two items, the fragment as it wrote it.
    private static let wordList = """
        <html><head><meta http-equiv=Content-Type content="text/html; charset=utf-8"></head>
        <body><!--StartFragment-->
        <p class=MsoListParagraphCxSpFirst style='text-indent:-18.0pt;mso-list:l0 level1 lfo1'><![if !supportLists]><span
        style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:
        Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
        </span></span></span><![endif]>Apples<o:p></o:p></p>
        <p class=MsoListParagraphCxSpLast style='text-indent:-18.0pt;mso-list:l0 level1 lfo1'><![if !supportLists]><span
        style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:
        Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
        </span></span></span><![endif]>Pears<o:p></o:p></p>
        <!--EndFragment--></body></html>
        """
}
