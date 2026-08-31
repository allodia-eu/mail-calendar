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
            showsTitle: hasContactsDetailPane
        )
    }

    /// The contacts detail column: the opened person, or the placeholder.
    @ViewBuilder var contactsDetailPane: some View {
        if let openedContact {
            ContactDetailView(
                detail: openedContact.detail,
                accountLabels: model.contactAccountLabels
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
}
