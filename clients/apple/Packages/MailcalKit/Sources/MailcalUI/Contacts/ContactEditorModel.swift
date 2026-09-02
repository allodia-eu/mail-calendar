// The contact editor's state and the intent it produces: a plain observable, so the validation
// and the create-versus-edit split are testable without building a view.
//
// The rule that is load-bearing here: **an edit names a card, never a person.** The list and the
// detail show people, which the core assembled from the cards several accounts hold, so an editor
// opened on a merged person has to say which card it is editing and be seeded from that card alone
// (docs/contacts.md §3). That is why `EditedCard` carries an account and a card id, not a row id.

import MailcalBindings
import Observation
import SwiftUI

/// The card an editor is editing (absent when creating).
struct EditedCard: Hashable {
    /// The person the row carried, so a card retired by a merge still resolves.
    let person: String
    let account: String
    let card: String
}

/// One address book a create can file into, labelled the way the user knows it.
///
/// The account's address is what a person recognises; the book's own name earns a place only where
/// one account offers several, where the address alone would repeat down the list.
struct ContactBookChoice: Identifiable, Hashable {
    let account: String
    let addressBook: String
    let label: String
    let isDefault: Bool

    var id: String { "\(account)\u{1F}\(addressBook)" }

    static func from(_ targets: [ContactTarget], accountLabels: [String: String])
        -> [ContactBookChoice] {
        var perAccount: [String: Int] = [:]
        for target in targets { perAccount[target.account, default: 0] += 1 }
        return targets.map { target in
            let account = accountLabels[target.account] ?? target.account
            let several = (perAccount[target.account] ?? 0) > 1
            return ContactBookChoice(
                account: target.account,
                addressBook: target.addressBook,
                label: several && !target.name.isEmpty ? "\(account) (\(target.name))" : account,
                isDefault: target.isDefault
            )
        }
    }
}

/// One card an edit could go to, labelled by the account the user knows it by.
struct ContactCardChoice: Identifiable, Hashable {
    let account: String
    let card: String
    let label: String

    var id: String { "\(account)\u{1F}\(card)" }

    static func from(_ cards: [ContactCardRef], accountLabels: [String: String])
        -> [ContactCardChoice] {
        cards.map {
            ContactCardChoice(
                account: $0.account,
                card: $0.card,
                label: accountLabels[$0.account] ?? $0.account
            )
        }
    }
}

/// Why a form cannot be saved, so the view can pick its sentence.
enum ContactFormError { case empty, email }

/// The mutable state of an open contact editor.
///
/// The validation is a **copy** of the core's, and deliberately so: the core refuses a card with
/// nothing to file it under, but it has no locale and cannot choose the sentence to put under the
/// form. The client decides what to say; the core stays the backstop.
/// `@MainActor` like `MailboxModel`: it is only ever read and mutated from a view, and marking it
/// is what keeps it out of the Sendable checking a shared class would otherwise need.
@MainActor
@Observable
final class ContactEditorModel: Identifiable {
    let id = UUID()
    /// The card being edited, or `nil` for a create.
    let editing: EditedCard?
    /// Where a create may file the contact. Empty on an edit, which files nowhere new.
    let targets: [ContactBookChoice]

    var givenName: String
    var surname: String
    var organization: String
    var title: String
    /// The addresses and numbers, in the order they are drawn: the first address is the person's
    /// primary one, which is what the avatar and the list row are keyed on. A contact with none
    /// opens with one empty row, so the field is something to type in rather than a heading over
    /// a button.
    var emails: [String]
    var phones: [String]
    var target: ContactBookChoice?
    /// Shown only after a Save that could not go through: a message under a field the user has
    /// not finished filling in is an accusation, not help.
    var showsError = false

    private init(editing: EditedCard?, targets: [ContactBookChoice], seed: ContactEdit?) {
        self.editing = editing
        self.targets = targets
        givenName = seed?.givenName ?? ""
        surname = seed?.surname ?? ""
        organization = seed?.organization ?? ""
        title = seed?.title ?? ""
        let seededEmails = seed?.emails ?? []
        let seededPhones = seed?.phones ?? []
        emails = seededEmails.isEmpty ? [""] : seededEmails
        phones = seededPhones.isEmpty ? [""] : seededPhones
        target = targets.first(where: \.isDefault) ?? targets.first
    }

    /// An empty form filing into the account's default book, else the first on offer.
    static func create(targets: [ContactBookChoice]) -> ContactEditorModel {
        ContactEditorModel(editing: nil, targets: targets, seed: nil)
    }

    /// A form seeded with one card's values.
    static func edit(_ card: EditedCard, seed: ContactEdit) -> ContactEditorModel {
        ContactEditorModel(editing: card, targets: [], seed: seed)
    }

    var isEditing: Bool { editing != nil }

    /// Whether the destination picker is a decision: one book is a fact, not a choice.
    var picksTarget: Bool { editing == nil && targets.count > 1 }

    /// What is wrong with the form, or `nil` when it can be saved.
    var error: ContactFormError? {
        let edit = trimmed
        if edit.givenName.isEmpty, edit.surname.isEmpty, edit.organization.isEmpty,
           edit.emails.isEmpty {
            return .empty
        }
        return edit.emails.contains(where: { !Self.isAddressShaped($0) }) ? .email : nil
    }

    /// The intent a Save dispatches, or `nil` when the form is not valid.
    func intent() -> Intent? {
        guard error == nil else { return nil }
        let edit = trimmed
        if let card = editing {
            return .updateContact(
                person: card.person, account: card.account, card: card.card, edit: edit
            )
        }
        return .createContact(
            account: target?.account, addressBook: target?.addressBook, edit: edit
        )
    }

    /// The form with every value trimmed and its blank rows dropped.
    ///
    /// The core trims too; doing it here as well is what makes the validation agree with the
    /// refusal. A form holding one empty address row is a form with no addresses, and telling the
    /// user otherwise would be a message about a row they can see is blank.
    private var trimmed: ContactEdit {
        let list = { (values: [String]) in
            values.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
        }
        return ContactEdit(
            givenName: givenName.trimmingCharacters(in: .whitespacesAndNewlines),
            surname: surname.trimmingCharacters(in: .whitespacesAndNewlines),
            organization: organization.trimmingCharacters(in: .whitespacesAndNewlines),
            title: title.trimmingCharacters(in: .whitespacesAndNewlines),
            emails: list(emails),
            phones: list(phones)
        )
    }

    /// Whether a string is shaped like an email address; the same test the core applies.
    ///
    /// A backstop, not a parser: the server is the authority on what it accepts. What this catches
    /// is the value that would reach it as a malformed card and come back as an opaque 400.
    static func isAddressShaped(_ value: String) -> Bool {
        guard let at = value.firstIndex(of: "@"), at != value.startIndex else { return false }
        let domain = value[value.index(after: at)...]
        return !domain.isEmpty && !domain.contains("@") && !domain.hasPrefix(".")
    }
}
