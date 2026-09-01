// The MAILCAL_APPEARANCE launch override's spellings, the lever a showcase or UI run pulls to
// photograph both themes without touching the machine's own setting. A value it silently ignores
// looks exactly like a working one in the resulting screenshot, so the rule is pinned rather than
// trusted. The spellings are a cross-client contract: scripts/dev/* pass the same three words to
// every platform.

import MailcalBindings
import SwiftUI
import XCTest

@testable import MailcalUI

final class AppearanceModeTests: XCTestCase {
    func testTheContractSpellingsAreMatchedLiterally() {
        XCTAssertEqual(AppearanceMode.parse("light"), .light)
        XCTAssertEqual(AppearanceMode.parse("dark"), .dark)
        // Trimmed and case-insensitive, like every other launch hook.
        XCTAssertEqual(AppearanceMode.parse(" DARK "), .dark)
        // "system" is an override in its own right, not an absent one: it pins a run to the host's
        // setting even for a developer whose stored choice is Light or Dark.
        XCTAssertEqual(AppearanceMode.parse("system"), .system)
    }

    func testAnythingElseLeavesTheStoredChoiceStanding() {
        XCTAssertNil(AppearanceMode.parse(nil))
        XCTAssertNil(AppearanceMode.parse(""))
        XCTAssertNil(AppearanceMode.parse("   "))
        XCTAssertNil(AppearanceMode.parse("night"))
        XCTAssertNil(AppearanceMode.parse("1"))
    }

    /// Only an explicit choice forces a scheme. `nil` is what leaves the hierarchy following the
    /// host, so a desktop that switches light/dark mid-session still reaches the app.
    func testFollowingTheHostForcesNoScheme() {
        XCTAssertNil(AppearanceMode.colorScheme(.system))
        XCTAssertEqual(AppearanceMode.colorScheme(.light), .light)
        XCTAssertEqual(AppearanceMode.colorScheme(.dark), .dark)
    }
}
