// The composer's recipient-field string handling (RecipientTokens.swift).
//
// Everything here is a rule `docs/contacts.md` §4 binds every client to, and every one of them
// fails *silently* when it is wrong: a query that never matches, recipients quietly deleted, spaces
// eaten out of a name. None of those raise an error, they just produce a composer that seems not
// to work, or a message sent to the wrong people.

import Foundation
import Testing

@testable import MailcalUI

@Suite struct RecipientTokenTests {

    @Test func theQueryIsTheLastTokenNotTheWholeField() {
        // The field holds a LIST. Query the whole thing and, the moment a first recipient is
        // entered, nothing can ever match again, the failure looks like "autosuggest stopped
        // working" rather than like a bug in string handling.
        #expect(currentRecipientToken("gr") == "gr")
        #expect(currentRecipientToken("ada@example.test, gr") == "gr")
        #expect(currentRecipientToken("") == "")
        // Ends at a separator: nothing is being typed, so there is nothing to suggest.
        #expect(currentRecipientToken("ada@example.test, ") == "")
    }

    @Test func acceptingASuggestionKeepsTheRecipientsAlreadyEntered() {
        // Replacing the whole field on selection destroys every address already entered, and the
        // user finds out when the message reaches one person instead of three.
        let field = acceptRecipientSuggestion("ada@example.test, gr", "grace@example.test")
        #expect(committedRecipients(field) == ["ada@example.test", "grace@example.test"])
        // The token is empty afterwards, so the input clears and the caret has nowhere to be but
        // the end, the structural version of "put the caret at the end after a programmatic
        // change".
        #expect(currentRecipientToken(field).isEmpty)
    }

    @Test func acceptingIntoAnEmptyFieldStillEndsAtASeparator() {
        let field = acceptRecipientSuggestion("ad", "ada@example.test")
        #expect(committedRecipients(field) == ["ada@example.test"])
        #expect(currentRecipientToken(field).isEmpty)
    }

    @Test func pillsAndTheTypedTokenRoundTripToTheFieldTheyCameFrom() {
        // The property that keeps the pills from silently altering recipients: they are a VIEW of
        // the field's string, never a second source of truth, so re-assembling them must return
        // exactly what was there.
        for field in [
            "",
            "ada@example.test",
            "ada@example.test, ",
            "ada@example.test, grace@example.test, ",
            "ada@example.test, gr",
            "ada@example.test, grace@example.test, ba",
        ] {
            let rebuilt = recipientFieldText(committedRecipients(field), currentRecipientToken(field))
            #expect(rebuilt == field, "field \(field.debugDescription) did not round-trip")
        }
    }

    @Test func aTrailingSpaceSurvivesTheRoundTripThroughTheParent() {
        // The space-eating bug, as the pure invariant the view's guard depends on. The token is
        // TRIMMED, so a field ending in a space and the token derived from it are not equal as
        // strings, a guard comparing them raw would re-seed the input on every space and turn
        // "John Smith" into "JohnSmith", with a name query that can then never match.
        let typed = "John "
        let field = recipientFieldText(committedRecipients(""), typed)
        #expect(field == typed)                            // the space reaches the field
        #expect(currentRecipientToken(field) == "John")    // …but not the token
        #expect(typed.trimmingCharacters(in: .whitespaces) == currentRecipientToken(field))
    }

    @Test func aPrefilledFieldHasNothingInProgress() {
        // The reply-all bug: the field arrives from the core, so the trailing-token rule, which
        // guesses what the user is typing, had the last recipient rendered as raw text beside one
        // pill, and a single-address Cc drawn as no pill at all.
        let to = seededRecipientField("bestuur@example.test, tc@example.test")
        #expect(committedRecipients(to) == ["bestuur@example.test", "tc@example.test"])
        #expect(currentRecipientToken(to).isEmpty)

        let cc = seededRecipientField("rene@example.test")
        #expect(committedRecipients(cc) == ["rene@example.test"])
        #expect(currentRecipientToken(cc).isEmpty)
    }

    @Test func seedingAnEmptyFieldLeavesItEmpty() {
        // A new message and a forward open with nothing. Send is gated on a non-blank To, so a
        // lone separator here would enable it over a message addressed to nobody.
        #expect(seededRecipientField("") == "")
        #expect(seededRecipientField("  ") == "")
        // Already committed: seeding it again changes nothing, so the draft-dirty comparison the
        // composer makes against the opening value cannot drift.
        let once = seededRecipientField("ada@example.test, grace@example.test")
        #expect(seededRecipientField(once) == once)
    }

    @Test func removingAPillDropsOnlyThatRecipient() {
        let field = "ada@example.test, grace@example.test, ba"
        #expect(removeRecipient(field, at: 0) == "grace@example.test, ba")
        #expect(removeRecipient(field, at: 1) == "ada@example.test, ba")
    }

    @Test func removingAPillThatIsNoLongerThereChangesNothing() {
        // The pill list and a click on it are a frame apart; a re-render in between must not
        // crash the composer.
        let field = "ada@example.test, gr"
        #expect(removeRecipient(field, at: 5) == field)
        #expect(removeRecipient(field, at: -1) == field)
    }

    @Test func blankAndDuplicateSeparatorsNeverBecomePills() {
        #expect(committedRecipients("") == [])
        #expect(committedRecipients(",,") == [])
        #expect(committedRecipients("ada@example.test,,grace@example.test, x")
            == ["ada@example.test", "grace@example.test"])
    }

    @Test func aPreFilledCcOrBccOpensTheCollapsedRow() {
        // Cc/Bcc open collapsed, so a pre-filled one has to force them open: a recipient the sender
        // cannot see is one they cannot remove (docs/composer-security.md, Gate 12).
        #expect(revealsCcBcc(cc: "carol@example.test", bcc: ""))
        #expect(revealsCcBcc(cc: "", bcc: "snoop@evil.test"))
        #expect(revealsCcBcc(cc: "carol@example.test", bcc: "snoop@evil.test"))
        #expect(!revealsCcBcc(cc: "", bcc: ""))
        // Whitespace is not an address.
        #expect(!revealsCcBcc(cc: "  ", bcc: " "))
    }

    @Test func theCaretOpensInTheBodyOnlyForAComposerThatIsAlreadyAddressed() {
        // One predicate, because exactly one of To and the body may take the caret and two flags
        // can disagree (docs/contacts.md §4).
        #expect(!RichComposeView.opensInBody(mode: .new, to: ""))
        #expect(!RichComposeView.opensInBody(mode: .new, to: "  "), "whitespace is not an address")
        // A mail link, or an assistant's draft: a new message that arrived addressed.
        #expect(RichComposeView.opensInBody(mode: .new, to: "bob@example.test"))
        #expect(RichComposeView.opensInBody(mode: .reply, to: "bob@example.test"))
        #expect(RichComposeView.opensInBody(mode: .replyAll, to: "bob@example.test"))
        // A forward carries the quoted original and no recipient. The body still takes the caret:
        // the note above the quote is what the user came to write, and Send already says the
        // message is unaddressed.
        #expect(RichComposeView.opensInBody(mode: .forward, to: ""))
    }

    @Test func theListClosesOnceTheTypedAddressIsTheSuggestion() {
        // Offering what is already typed covers the next field for nothing.
        #expect(shouldShowRecipientSuggestions("ad", ["ada@example.test"]))
        #expect(!shouldShowRecipientSuggestions("ada@example.test", ["ada@example.test"]))
        #expect(!shouldShowRecipientSuggestions("ADA@example.test", ["ada@example.test"]))
        // A blank token asks nothing, so there is nothing to show.
        #expect(!shouldShowRecipientSuggestions("ada@example.test, ", ["grace@example.test"]))
        #expect(!shouldShowRecipientSuggestions("ad", []))
    }
}
