// The shell's contacts surface: the list column, the detail pane, and opening one person.
//
// Contacts follow the mail layout rather than the calendar's: a list beside a detail on macOS and
// iPad, and a push on iPhone. That is what the data is, a list of people you pick one of, where
// the calendar is one wide grid with nothing to select beside it.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// The contacts list column. Search goes into the **core**, not a filter over the rows already
    /// loaded: the core matches name, email, phone, organisation and title, so every client narrows
    /// identically, and a person beyond the page cap is still findable (docs/contacts.md §2).
    var contactsList: some View {
        ContactsListView(
            rows: model.contacts,
            onSearch: { query in model.searchContacts(query) },
            onOpen: { row in openContact(row) },
            // No row is highlighted on iPhone: opening pushes a screen rather than filling a
            // second column, so there is nothing beside the list to point at.
            selectedID: hasContactsDetailPane ? openedContact?.id : nil,
            // Same distinction, second consequence: the phone's navigation bar already says
            // "Contacts", so the list must not say it again directly underneath.
            showsTitle: hasContactsDetailPane,
            canCreate: !contactTargets.isEmpty,
            onCreate: { contactEditor = .create(targets: contactTargets) },
            writeLine: model.contactWriteLine
        )
        .task { await loadContactTargets() }
        .sheet(item: $contactEditor) { editor in
            ContactEditorView(
                model: editor,
                onSave: { intent in
                    contactEditor = nil
                    model.app?.dispatch(intent: intent)
                },
                onCancel: { contactEditor = nil }
            )
        }
        .sheet(isPresented: choosingContactCard) {
            ContactCardChoiceView(
                cards: contactCardChoice ?? [],
                onPick: { card in
                    contactCardChoice = nil
                    if let person = openedContact?.id { openContactEditor(person: person, card: card) }
                },
                onCancel: { contactCardChoice = nil }
            )
        }
    }

    /// Whether the "which account?" question is open. A binding rather than `sheet(item:)`
    /// because the value is a *list*, which has no identity of its own to key a sheet on.
    private var choosingContactCard: Binding<Bool> {
        Binding(get: { contactCardChoice != nil }, set: { if !$0 { contactCardChoice = nil } })
    }

    /// The contacts detail column: the opened person, or the placeholder.
    @ViewBuilder var contactsDetailPane: some View {
        if let openedContact {
            ContactDetailView(
                detail: openedContact.detail,
                accountLabels: model.contactAccountLabels,
                onEdit: openedContact.detail.editableCards.isEmpty
                    ? nil
                    : { beginEditContact(openedContact.detail) }
            )
        } else {
            ContactDetailPlaceholder()
        }
    }

    /// Whether this platform shows the contact detail beside the list rather than pushing it.
    var hasContactsDetailPane: Bool {
        #if os(macOS)
        return true
        #else
        return hSize != .compact
        #endif
    }

    /// Opens one person's detail.
    ///
    /// The lookup blocks on the core's runtime and lands on the store's connection thread, so it is
    /// awaited off the main thread rather than read inline while rendering. A `nil` answer means the
    /// person is genuinely gone, never merely renumbered, since the core keeps ids retired by a
    /// merge pointing at the surviving person, so the selection is cleared rather than left
    /// showing someone the list no longer holds.
    func openContact(_ row: ContactRow) {
        Task { @MainActor in
            let detail = await model.contactDetail(row.id)
            openedContact = detail.map(OpenedContact.init(detail:))
        }
    }

    /// Reads the writable address books, off the main thread for the same reason as the detail.
    ///
    /// Read on entering the surface rather than when the create button is pressed, because the
    /// answer decides whether that button exists at all.
    func loadContactTargets() async {
        contactTargets = ContactBookChoice.from(
            await model.contactTargets(),
            accountLabels: model.contactAccountLabels
        )
    }

    /// The Edit button beside an open person.
    ///
    /// One editable card opens straight into the form. Several is a question only the user can
    /// answer, because a person is several accounts' cards and an edit writes to exactly one of
    /// them (docs/contacts.md §3).
    func beginEditContact(_ detail: ContactDetail) {
        let cards = ContactCardChoice.from(
            detail.editableCards,
            accountLabels: model.contactAccountLabels
        )
        if cards.count == 1 {
            openContactEditor(person: detail.id, card: cards[0])
        } else if cards.count > 1 {
            contactCardChoice = cards
        }
    }

    /// Reads one card's values, then opens the editor on them.
    ///
    /// Seeded from the **card** and never from the person on screen: the person is a merge, so its
    /// values belong to different accounts' cards, and saving them into one would file the work
    /// address book's details in the personal one. A card that has gone (a sync deleted it between
    /// the tap and the read) opens no editor: seeding one from nothing would offer to save a blank
    /// card over it.
    func openContactEditor(person: String, card: ContactCardChoice) {
        Task { @MainActor in
            guard let seed = await model.contactCard(
                person: person, account: card.account, card: card.card
            ) else { return }
            contactEditor = .edit(
                EditedCard(person: person, account: card.account, card: card.card),
                seed: seed
            )
        }
    }
}
