// Reaching a Settings category from the showcase's Settings screen, on the iPhone's hub and in the
// iPad's sidebar alike, and bringing a control on a long category into view.

import UIKit
import XCTest

@MainActor
enum SettingsCategories {
    /// Launches on Settings with `hooks` and opens the category called `name`.
    static func open(_ name: String, hooks: [String: String] = [:]) throws -> App {
        let app = try ShowcaseApp.launch(hooks.merging(["MAILCAL_SHOWCASE_SCREEN": "settings"]) { $1 })
        try app.wait(.text("Settings").role("AXHeading"), "Settings did not open")
        var found = try row(name, in: app)
        for _ in 0..<4 where found == nil {
            app.input.swipeUp()
            found = try row(name, in: app)
        }
        try app.tap(try XCTUnwrap(found, "Settings has no \(name) row"))
        return app
    }

    /// The category's row: on the iPhone's hub a button whose label leads with the name, in the
    /// iPad's sidebar the name alone, as text. `nil` while it is not drawn.
    static func row(_ name: String, in app: App) throws -> Element? {
        if ShowcaseApp.pad {
            return try app.query(.text(name)).first
        }
        return try app.query(.textLike(name).role("AXButton")).first { $0.label.hasPrefix("\(name), ") }
    }

    /// Swipes the category up until `filter` is on screen, and returns it.
    @discardableResult
    static func reveal(_ filter: Filter, in app: App) throws -> Element {
        for _ in 0..<8 where try !app.exists(filter) {
            app.input.swipeUp()
            try app.settle()
        }
        return try app.wait(filter, "\(filter) is not on the page")
    }
}
