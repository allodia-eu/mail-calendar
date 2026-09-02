// The rule a dismissed sheet has to obey: it stays dismissed.
//
// Driven against `sheetItemBinding` rather than a presented sheet, because `swift test` cannot
// present one and there is no Apple UI-test target. The bug this pins was found by hand on an
// iPhone: Cancel and Save both dismissed the event editor and both immediately reopened it.

import SwiftUI
import Testing

@testable import MailcalUI

@MainActor struct SheetItemBindingTests {
    /// While the sheet is up the binding is an ordinary one: reads see storage, writes reach it.
    @Test func aPresentedSheetReadsAndWritesItsItem() {
        var stored: String? = "open"
        let binding = sheetItemBinding(
            Binding(get: { stored }, set: { stored = $0 }),
            presented: "handed over"
        )

        #expect(binding.wrappedValue == "open")
        binding.wrappedValue = "edited"
        #expect(stored == "edited")
    }

    /// The regression. The content is still alive while the sheet animates away and it still
    /// writes; that write must not put the item back, or the sheet presents itself again.
    @Test func aWriteAfterDismissalDoesNotBringTheSheetBack() {
        var stored: String? = "open"
        let binding = sheetItemBinding(
            Binding(get: { stored }, set: { stored = $0 }),
            presented: "handed over"
        )

        // Cancel or Save.
        stored = nil
        // The dismissing content commits a field, resigns focus, settles a picker.
        binding.wrappedValue = "a late write"

        #expect(stored == nil, "a dismissed sheet must stay dismissed")
    }

    /// The read still falls back, which is the other half of the job: SwiftUI evaluates a sheet's
    /// content on the frame where the item has just been cleared, and force-unwrapping traps.
    @Test func aClearedItemStillReadsAsTheValueTheSheetWasGiven() {
        var stored: String?
        let binding = sheetItemBinding(
            Binding(get: { stored }, set: { stored = $0 }),
            presented: "handed over"
        )

        #expect(binding.wrappedValue == "handed over")
    }
}
