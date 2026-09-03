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

    /// Every address book a new contact could be saved into, across every account.
    ///
    /// The "save to…" picker, and the answer to whether a create may be offered at all: an empty
    /// list means this user has nowhere to put one. Off the main thread for the same reason as
    /// `contactDetail`.
    func contactTargets() async -> [ContactTarget] {
        guard let app else { return [] }
        return await Task.detached(priority: .userInitiated) { app.contactTargets() }.value
    }

    /// The editable values of one source card, for seeding an editor.
    ///
    /// Read from the **card**, never from the person the detail showed: the person is a merge, so
    /// seeding an editor from it would offer another account's values for saving into this one's
    /// address book.
    func contactCard(person: String, account: String, card: String) async -> ContactEdit? {
        guard let app else { return nil }
        return await Task.detached(priority: .userInitiated) {
            app.contactCard(person: person, account: account, card: card)
        }.value
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

    /// What the contacts list says about the most recent create or edit.
    ///
    /// `.failed` means "we could not confirm this saved", never "rejected": a write whose server
    /// call succeeded and whose reconcile did not has already landed, and the next sync heals the
    /// local copy. `.invalid` is stated under the form the user is still looking at, so nothing
    /// repeats it here.
    var contactWriteLine: String? {
        switch contactWriteStatus {
        case .saving: L10n.contacts_saving()
        case .saved: L10n.contacts_saved()
        case .failed: L10n.contacts_save_unconfirmed()
        default: nil
        }
    }

    /// Account id → the address the user knows that account by, for the detail view's provenance
    /// labels. The core's ids are internal (`alice@test.local@jmap:127.0.0.1:18080`); showing one
    /// is both ugly and a leak of how ids are built. An id with no account left falls back to
    /// itself rather than vanishing.
    var contactAccountLabels: [String: String] {
        Dictionary(accounts.map { ($0.id, $0.email) }, uniquingKeysWith: { first, _ in first })
    }
}
