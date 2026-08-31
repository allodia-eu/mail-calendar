// The contacts surface's model calls: the list, one person's detail, the search query, and the
// composer's recipient autosuggest.
//
// The list is an ordinary pushed snapshot (`Surface.contacts` → `contactList()`), read on the main
// actor like the mailbox and the agenda. **The other two are not.** `contactDetail` and
// `recipientSuggestions` are direct queries that block the calling thread on the core's runtime and
// land on the store's connection thread, so a call made while a sync holds that connection waits
// for it. The FFI's own doc comment says as much: a host must keep them off its UI thread. They are
// therefore `async` here and hop to a detached task, and the per-keystroke one is debounced at its
// call site (RecipientField.swift) rather than fired per character.
//
// Search runs in the CORE, not as a filter over the rows already on screen: the core matches name,
// email, phone, organisation and title, so every client narrows identically, and a person beyond
// the 200-row page cap is still findable.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Switch to Contacts: clear any stale search, paint what is already cached, then sync.
    ///
    /// The query is cleared in the **core** first. A client's own search field is view state that
    /// dies with the view, but the query lives in the core, so without this, leaving Contacts
    /// mid-search and coming back shows a filtered list under an empty search box: a narrowing the
    /// user can no longer see, which is the failure `docs/search.md` exists to prevent.
    func showContacts() {
        destination = .contacts
        app?.dispatch(intent: .searchContacts(query: ""))
        // Paint the cached list immediately, then sync, so switching tabs never shows an empty
        // screen while the network is consulted.
        if let snapshot = app?.contactList() { contacts = snapshot.rows }
        app?.dispatch(intent: .refreshContacts)
    }

    /// Narrows the contacts list. The core answers with a `Surface::Contacts` signal; an empty
    /// query resets it to the whole list.
    func searchContacts(_ query: String) {
        app?.dispatch(intent: .searchContacts(query: query))
    }

    /// The detail of one person, by the id a `ContactRow` carries.
    ///
    /// `nil` means the person is genuinely gone, never merely renumbered. Merging retires ids and
    /// the core keeps the retired ones pointing at the surviving person, so a row still held after
    /// a background sync merged it opens fine without refreshing the list first.
    func contactDetail(_ id: String) async -> ContactDetail? {
        guard let app else { return nil }
        return await Task.detached(priority: .userInitiated) { app.contactDetail(id: id) }.value
    }

    /// Ranked address suggestions for a partially-typed recipient.
    ///
    /// Draws on synced contacts **and** on people the user has written to before (the engine mines
    /// the Sent mailbox), so it is useful on an account with no address book at all, which is most
    /// accounts. A blank query returns nothing: a dropdown of everyone you have ever emailed, the
    /// moment To takes focus, is noise rather than help.
    func recipientSuggestions(_ query: String) async -> [RecipientMatch] {
        guard let app else { return [] }
        return await Task.detached(priority: .userInitiated) {
            app.recipientSuggestions(query: query)
        }.value
    }

    /// Account id → the address the user knows that account by, for the detail view's provenance
    /// labels. The core's ids are internal (`alice@test.local@jmap:127.0.0.1:18080`); showing one
    /// is both ugly and a leak of how ids are built. An id with no account left falls back to
    /// itself rather than vanishing.
    var contactAccountLabels: [String: String] {
        Dictionary(accounts.map { ($0.id, $0.email) }, uniquingKeysWith: { first, _ in first })
    }
}
