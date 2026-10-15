// Where a recipient or invitee field's suggestion list goes on a phone or tablet, tested without a
// view: under the field and no taller than the room above the keyboard, or above the field where
// that room is too small (docs/contacts.md §4). The same cases as Android's
// `RecipientPopupPositionTest`, in points.

import CoreGraphics
import Testing

@testable import MailcalUI

@Suite struct RecipientSuggestionPlacementTests {
    /// An iPhone in points: content from under the navigation bar to the top of the keyboard.
    private let visibleTop: CGFloat = 100
    private let keyboardTop: CGFloat = 520

    private func placement(_ field: CGRect) -> RecipientSuggestionPlacement {
        recipientSuggestionPlacement(field: field, visibleTop: visibleTop, visibleBottom: keyboardTop)
    }

    @Test func aFieldNearTheTopOpensBelowAndStopsAtTheKeyboard() {
        let chosen = placement(CGRect(x: 16, y: 150, width: 300, height: 36))
        #expect(!chosen.above)
        #expect(chosen.maxHeight == CGFloat(520 - 186 - 4))
    }

    @Test func aFieldRestingOnTheKeyboardOpensAboveIt() {
        // The event editor's invitee field: the form scrolls it to just above the keyboard.
        let chosen = placement(CGRect(x: 16, y: 470, width: 300, height: 36))
        #expect(chosen.above)
        #expect(chosen.maxHeight == CGFloat(470 - 100 - 4))
    }

    @Test func enoughRoomBelowWinsEvenWhereThereIsMoreAbove() {
        // Under the field is where the eye already is; the list moves only when that room is
        // too small for about three suggestions.
        let chosen = placement(CGRect(x: 16, y: 320, width: 300, height: 36))
        #expect(!chosen.above)
        #expect(chosen.maxHeight == CGFloat(160))
    }

    @Test func lessRoomBelowThanAboveButNotEnoughForThreeOpensAbove() {
        let chosen = placement(CGRect(x: 16, y: 340, width: 300, height: 36))
        #expect(chosen.above)
    }

    @Test func aFieldUnderTheKeyboardNeverAsksForANegativeHeight() {
        let chosen = placement(CGRect(x: 16, y: 700, width: 300, height: 36))
        #expect(chosen.above)
        #expect(chosen.maxHeight >= 0)

        let squeezed = recipientSuggestionPlacement(
            field: CGRect(x: 16, y: 100, width: 300, height: 440), visibleTop: 100, visibleBottom: 520
        )
        #expect(squeezed.maxHeight >= 0)
    }
}
