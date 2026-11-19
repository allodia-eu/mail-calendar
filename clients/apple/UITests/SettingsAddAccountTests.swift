// Adding a later account (`docs/settings.md`, row 11): Settings → Accounts carries the one add, and
// the mailbox carries none.
//
// The add closes Settings before the setup sheet opens, because a sheet presented over Settings is
// silently dropped. No package test sees that: it is two modals and their order, so it is driven
// here the way a person drives it.

import UIKit
import XCTest

@MainActor
final class SettingsAddAccountTests: XCTestCase {
    func testAccountsSettingsOpensAccountSetup() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"])
        try app.wait(.text("Settings").role("AXHeading"), "Settings did not open")
        // The iPhone hub row reads its summary after the name; the iPad sidebar row is the name.
        // The hub is a lazy list taller than the screen, so its Accounts row exists only once it
        // has been scrolled to.
        var accounts = try accountsRow(app)
        for _ in 0..<3 where accounts == nil {
            app.input.swipeUp()
            accounts = try accountsRow(app)
        }
        try app.tap(try XCTUnwrap(accounts, "Settings has no Accounts row"))
        try app.press(.id("settings-add-account"))
        try app.wait(
            .text("you@example.com").role("AXTextField"),
            "Add account in Settings did not open account setup"
        )
    }

    func testTheMailboxHasNoAddOfItsOwn() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        // The folder list is lazy, so its last row exists only once it has been scrolled to: on
        // iPhone that means opening the drawer first.
        if !ShowcaseApp.pad {
            try app.press(.button(ShowcaseApp.folders))
        }
        // The folder list is the leftmost list on screen: the drawer, or the iPad's sidebar.
        let lists = try app.waitAll(Filter().type("UICollectionView"), "no folder list")
        let folders = try XCTUnwrap(
            lists.min { ($0.frame?.minX ?? .infinity) < ($1.frame?.minX ?? .infinity) },
            "no folder list"
        )
        // Swiped with a finger: a SwiftUI list does not take VoiceOver's scroll.
        for _ in 0..<2 {
            let centre = try app.point(folders)
            centre.press(
                forDuration: 0.05, thenDragTo: centre.withOffset(CGVector(dx: 0, dy: -400)),
                withVelocity: .fast, thenHoldForDuration: 0
            )
            try app.settle()
        }
        XCTAssertFalse(
            try app.query(.textLike("Add account").hidden())
                .contains { $0.role == "AXButton" && $0.label.hasPrefix("Add account") },
            "a later account is added from Settings alone"
        )
    }

    /// The Accounts row on screen, or nil while the hub has not drawn it. The iPad's sidebar row
    /// reads as its name alone, as text.
    private func accountsRow(_ app: App) throws -> Element? {
        if ShowcaseApp.pad {
            return try app.query(.text("Accounts")).first
        }
        return try app.query(.textLike("Accounts").role("AXButton"))
            .first { $0.label.hasPrefix("Accounts") }
    }
}
