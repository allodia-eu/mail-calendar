// The unread count on the app's icon (docs/app-badge.md): what the Dock tile says for a count.

import Testing

@testable import MailcalUI

@Suite struct AppBadgeTests {
    @Test func zeroUnreadShowsNoBadge() {
        // Nothing unread and a provider that reports no count are both zero, and neither has a
        // number worth drawing (docs/folder-pane.md, rule 6).
        #expect(AppBadge.label(0) == nil)
    }

    @Test func theBadgeIsTheWholeCount() {
        #expect(AppBadge.label(1) == "1")
        #expect(AppBadge.label(548) == "548")
    }
}
