// A recipient field's undo and redo (RecipientHistory.swift, docs/undo.md rule 9): a step per
// address, never the input's own history, which would type a finished address back in.

import Foundation
import Testing

@testable import MailcalUI

@Suite struct RecipientHistoryTests {

    /// Types `text` one character at a time, as the input reports it.
    private func type(_ text: String, into history: inout RecipientHistory) {
        for letter in text {
            history.record(history.current + String(letter), typed: true)
        }
    }

    @Test func oneAddressTypedIsOneStep() {
        var history = RecipientHistory("")
        type("bob@example.test", into: &history)
        #expect(history.undo() == "")
        #expect(history.undo() == nil)
    }

    @Test func finishingAnAddressIsAStepOfItsOwn() {
        var history = RecipientHistory("")
        type("bob@example.test,", into: &history)
        type(" carol", into: &history)
        #expect(history.undo() == "bob@example.test,")
        #expect(history.undo() == "bob@example.test")
    }

    @Test func aRemovedRecipientComesBackAndGoesAgain() {
        var history = RecipientHistory("bob@example.test, carol@example.test, ")
        history.record(removeRecipient(history.current, at: 0), typed: false)
        #expect(history.undo() == "bob@example.test, carol@example.test, ")
        #expect(history.redo() == "carol@example.test, ")
    }

    @Test func anAcceptedSuggestionIsOneStep() {
        var history = RecipientHistory("")
        type("car", into: &history)
        history.record(acceptRecipientSuggestion(history.current, "carol@example.test"), typed: false)
        #expect(history.undo() == "car")
        #expect(history.undo() == "")
    }

    @Test func aChangeAfterAnUndoLeavesNothingToRedo() {
        var history = RecipientHistory("")
        history.record("bob@example.test, ", typed: false)
        _ = history.undo()
        history.record("carol@example.test, ", typed: false)
        #expect(!history.canRedo)
        #expect(history.redo() == nil)
    }

    @Test func typingAfterAnUndoStartsANewStep() {
        var history = RecipientHistory("")
        type("bo", into: &history)
        _ = history.undo()
        type("al", into: &history)
        #expect(history.undo() == "")
        #expect(history.undo() == nil)
    }

    @Test func aValueFromOutsideIsWhereTheHistoryStarts() {
        var history = RecipientHistory("")
        history.record("bob@example.test, ", typed: false)
        history.restart("ada@example.test, ")
        #expect(history.current == "ada@example.test, ")
        #expect(!history.canUndo)
    }

    @Test func keepsTheNewestHundredSteps() {
        var history = RecipientHistory("")
        for index in 0...RecipientHistory.maxSteps {
            history.record("r\(index)@example.test, ", typed: false)
        }
        var undone = 0
        while history.undo() != nil { undone += 1 }
        #expect(undone == RecipientHistory.maxSteps)
    }
}
