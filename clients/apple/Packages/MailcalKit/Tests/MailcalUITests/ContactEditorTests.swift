// What the contact editor turns a form into, and what it refuses (ContactEditorModel.swift).
//
// The claim worth gating is the one a plausible implementation gets wrong in silence: an edit
// names the CARD it was opened on, and a person is several cards. The rest is in service of it,
// because each is a way to end up saving into the wrong address book.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite @MainActor struct ContactEditorTests {

    private func target(
        _ account: String,
        _ book: String,
        name: String,
        isDefault: Bool = false
    ) -> ContactTarget {
        ContactTarget(account: account, addressBook: book, name: name, isDefault: isDefault)
    }

    private var labels: [String: String] {
        ["personal": "me@example.test", "work": "me@work.test"]
    }

    /// The picker opens on the account's own default book, not on whichever came first.
    @Test func aCreateOpensOnTheDefaultBookAndFilesTheChosenOne() {
        let books = ContactBookChoice.from(
            [
                target("personal", "personal-book", name: "Personal"),
                target("work", "work-book", name: "Work", isDefault: true),
            ],
            accountLabels: labels
        )
        let model = ContactEditorModel.create(targets: books)
        #expect(model.target?.account == "work")
        #expect(model.picksTarget)

        model.givenName = "Grace"
        model.surname = "Hopper"
        model.emails[0] = "grace@example.test"
        model.target = books.first { $0.account == "personal" }
        guard let intent = model.intent(),
              case let .createContact(account, addressBook, edit) = intent else {
            Issue.record("a valid form did not produce a create")
            return
        }
        #expect(account == "personal")
        #expect(addressBook == "personal-book")
        #expect(edit.emails == ["grace@example.test"])
    }

    /// One book is a fact, not a decision, so the picker is not shown for it.
    @Test func oneAddressBookIsNotAChoice() {
        let books = ContactBookChoice.from(
            [target("personal", "personal-book", name: "Personal", isDefault: true)],
            accountLabels: labels
        )
        #expect(!ContactEditorModel.create(targets: books).picksTarget)
    }

    /// A book's own name earns a place only where one account offers several.
    @Test func aBookIsLabelledByItsAccountAndByItsNameOnlyWhereThatRepeats() {
        let one = ContactBookChoice.from(
            [
                target("personal", "personal-book", name: "Personal"),
                target("work", "work-book", name: "Work"),
            ],
            accountLabels: labels
        )
        #expect(one.map(\.label) == ["me@example.test", "me@work.test"])

        let several = ContactBookChoice.from(
            [
                target("work", "work-book", name: "Personal"),
                target("work", "team-book", name: "Team"),
            ],
            accountLabels: labels
        )
        #expect(several.map(\.label) == ["me@work.test (Personal)", "me@work.test (Team)"])
    }

    /// An edit names the card it was opened on, never the person: a person is several accounts'
    /// cards, and saving without naming one files the work details in the personal book.
    @Test func anEditCarriesTheCardItWasOpenedOn() {
        let model = ContactEditorModel.edit(
            EditedCard(person: "7", account: "work", card: "c-work"),
            seed: ContactEdit(
                givenName: "Ada",
                surname: "Lovelace",
                organization: "",
                title: "",
                emails: ["ada@example.test"],
                phones: []
            )
        )
        #expect(!model.picksTarget)
        model.surname = "King"
        guard let intent = model.intent(),
              case let .updateContact(person, account, card, edit) = intent else {
            Issue.record("a valid form did not produce an update")
            return
        }
        #expect(person == "7")
        #expect(account == "work")
        #expect(card == "c-work")
        #expect(edit.surname == "King")
    }

    /// A company contact has no person's name; a card with none of the three is a blank row.
    @Test func anOrganizationAloneIsEnoughAndNothingAtAllIsNot() {
        let model = ContactEditorModel.create(targets: [])
        #expect(model.error == .empty)
        #expect(model.intent() == nil)
        model.organization = "Analytical Engines"
        #expect(model.error == nil)
    }

    /// The two refusals are different sentences on screen, so they are different values here.
    @Test func aMalformedAddressIsItsOwnRefusal() {
        for malformed in ["ada", "@example.test", "ada@", "ada@@example.test", "ada@.test"] {
            let model = ContactEditorModel.create(targets: [])
            model.givenName = "Ada"
            model.emails[0] = malformed
            #expect(model.error == .email, "\(malformed) was accepted")
        }
    }

    /// A row the user emptied is a row they removed: it must not fail validation as a blank
    /// address, and must not reach the core as one.
    @Test func blankRowsAreDroppedRatherThanRefused() {
        let model = ContactEditorModel.create(targets: [])
        model.givenName = "Ada"
        model.emails = ["  ", " ada@example.test "]
        model.phones = [""]
        guard let intent = model.intent(),
              case let .createContact(_, _, edit) = intent else {
            Issue.record("a form with an emptied row was refused")
            return
        }
        #expect(edit.emails == ["ada@example.test"])
        #expect(edit.phones.isEmpty)
    }

    /// A contact with no addresses opens with one empty row, so the field is something to type
    /// into rather than a heading over a button; one that has them opens on what it has.
    @Test func theValueListsOpenOnWhatTheCardHoldsOrOnOneEmptyRow() {
        #expect(ContactEditorModel.create(targets: []).emails == [""])

        let seeded = ContactEditorModel.edit(
            EditedCard(person: "1", account: "work", card: "c1"),
            seed: ContactEdit(
                givenName: "Ada",
                surname: "",
                organization: "",
                title: "",
                emails: ["a@example.test", "b@example.test"],
                phones: []
            )
        )
        #expect(seeded.emails == ["a@example.test", "b@example.test"])
        #expect(seeded.phones == [""])
    }

    /// A card is labelled by the account the user knows it by, never by the core's internal id.
    @Test func aCardChoiceIsLabelledByItsAccount() {
        let cards = ContactCardChoice.from(
            [
                ContactCardRef(account: "work", card: "c-work"),
                ContactCardRef(account: "unknown", card: "c-gone"),
            ],
            accountLabels: labels
        )
        #expect(cards.map(\.label) == ["me@work.test", "unknown"])
    }
}
