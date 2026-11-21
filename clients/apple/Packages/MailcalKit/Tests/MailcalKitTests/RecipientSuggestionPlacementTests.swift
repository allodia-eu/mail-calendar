// Where a recipient or invitee field's suggestion list goes, tested without a view: under the
// field and no taller than the room above the keyboard or a sheet's bar, or above the field where
// that room is too small (docs/contacts.md §4). The phone cases are Android's
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

    @Test func theInviteeFieldAtTheFootOfAMacSheetOpensAboveItsBar() {
        // The event editor on a Mac: the field is the sheet's last, and the bar with Cancel and
        // Save starts 40 points under it.
        let field = CGRect(x: 20, y: 318, width: 345, height: 22)
        let chosen = recipientSuggestionPlacement(field: field, visibleTop: 0, visibleBottom: 380)
        #expect(chosen.above)
        let box = chosen.box(field: field, minX: field.minX, width: field.width)
        #expect(box == CGRect(x: 20, y: 0, width: 345, height: 318 - 4))
    }

    @Test func theListStaysInTheRoomWhereverTheFieldIs() {
        for y in stride(from: visibleTop, through: keyboardTop - 36, by: 8) {
            let field = CGRect(x: 16, y: y, width: 300, height: 36)
            let box = placement(field).box(field: field, minX: 16, width: 300)
            #expect(box.minY >= visibleTop, "a field at \(y) put its list over the bar")
            #expect(box.maxY <= keyboardTop, "a field at \(y) put its list under the keyboard")
            #expect(!box.intersects(field), "a field at \(y) is covered by its own list")
        }
    }
}
