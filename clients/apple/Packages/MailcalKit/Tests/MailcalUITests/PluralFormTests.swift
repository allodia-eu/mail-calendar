// Plural forms resolve through the generated accessor, not at the call site.
//
// The catalog carries a `<key>_one` beside the plural and `mailcal-l10n` folds the pair into one
// accessor that chooses by count, so a count of one never reads "1 conversations". These assert
// the behaviour end to end, over the real catalog: the codegen's own tests pin the *rule*, and
// this pins that the app actually gets it.
//
// English only, deliberately: the point is which grammatical form the count picks, not how Dutch
// spells it. The locale a test process resolves to is the host's, and the wording of a translation
// is the translator's business, not this suite's.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct PluralFormTests {

    @Test func aCountOfOneTakesTheSingular() {
        #expect(L10n.mailbox_count_conversations(count: 1) == "1 conversation")
        #expect(L10n.mailbox_count_messages(count: 1) == "1 message")
        #expect(L10n.mailbox_count_results(count: 1) == "1 result")
    }

    @Test func everyOtherCountTakesThePlural() {
        // Zero included: an empty folder still states its count, and English reads it as a plural.
        #expect(L10n.mailbox_count_conversations(count: 0) == "0 conversations")
        #expect(L10n.mailbox_count_conversations(count: 2) == "2 conversations")
        #expect(L10n.mailbox_count_messages(count: 13) == "13 messages")
    }

    @Test func aSelectionOfOneTakesTheSingularToo() {
        #expect(L10n.selection_selected_messages(count: 1) == "1 message selected")
        #expect(L10n.selection_selected_conversations(count: 1) == "1 conversation selected")
        #expect(L10n.selection_selected_messages(count: 2) == "2 messages selected")
    }

    /// A reminder an hour or a day before is an ordinary choice, so these render at one routinely.
    @Test func reminderOffsetsOfOneTakeTheSingular() {
        #expect(L10n.event_reminder_minutes(count: 1) == "1 minute before")
        #expect(L10n.event_reminder_hours(count: 1) == "1 hour before")
        #expect(L10n.event_reminder_days(count: 1) == "1 day before")
        #expect(L10n.event_reminder_hours(count: 2) == "2 hours before")
    }

    /// The older pairs in the catalog spell the numeral into the sentence and are chosen between
    /// at the call site. Folding them into the plural mechanism would delete these accessors, so
    /// this is the guard that they still exist and still say what they say.
    @Test func aPairThatSpellsTheNumeralInKeepsItsOwnAccessor() {
        #expect(L10n.invitation_attendees_one() == "1 attendee")
        #expect(L10n.invitation_conflicts_one() == "1 other thing in your calendar then")
    }
}
