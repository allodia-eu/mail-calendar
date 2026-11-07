// The contacts list: a search field, an A–Z list of unified people, and its section headers.
//
// Every row here is one PERSON, not one provider card, the core has already merged the cards that
// share an address, across accounts. A merged row says so ("In 2 accounts"), which is a
// cross-platform product rule rather than a decoration: a user who filed a contact twice and now
// sees it once must be able to find out why (docs/contacts.md §1).
//
// Contacts can be created and edited here, and both affordances are CONDITIONAL: the create
// button only where there is a writable address book to file one in, the edit button only where
// this person has a card that can be written (docs/contacts.md §3).

import MailcalBindings
import SwiftUI

/// The contacts list.
///
/// `rows` is the core's already-ordered, already-filtered snapshot, this view does no sorting and
/// no matching of its own, so every client agrees on both. `onSearch` pushes the query **into the
/// core**, which is what lets a person beyond the 200-row page cap still be found.
struct ContactsListView: View {
    let rows: [ContactRow]
    let onSearch: (String) -> Void
    let onOpen: (ContactRow) -> Void
    /// The person shown in the detail pane, to highlight their row. Always `nil` on iPhone, where
    /// opening pushes a screen instead of filling a second column.
    var selectedID: String?
    /// Whether the list draws its own "Contacts" heading. True where it is one column of a wider
    /// window and nothing else names it; false inside a navigation stack, whose bar already does:
    /// two "Contacts" one above the other is chrome, not information.
    var showsTitle = true
    /// Whether there is anywhere at all to save a contact. No writable address book, no create
    /// button: offering one produces a save that fails after the user has typed everything in.
    var canCreate = false
    var onCreate: (() -> Void)?
    /// A word about the most recent write, or `nil` when there is nothing to say.
    var writeLine: String?

    @State private var query = ""

    var body: some View {
        VStack(spacing: 0) {
            header
            if let writeLine {
                Text(writeLine)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }
            Divider()
            if rows.isEmpty {
                emptyState
            } else {
                list
            }
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            if showsTitle {
                Text(L10n.contacts_title())
                    .font(.headline)
                    .lineLimit(1)
                    .frame(minWidth: 72, maxWidth: .infinity, alignment: .leading)
            }
            HStack(spacing: 4) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField(L10n.contacts_search_placeholder(), text: $query)
                    .textFieldStyle(.roundedBorder)
                    .frame(minWidth: 140, idealWidth: 180, maxWidth: 220)
                    .onChange(of: query) { _, typed in onSearch(typed) }
                if !query.isEmpty {
                    Button {
                        query = ""
                        // Clearing resets the filter in the CORE as well as the field, as one
                        // action: a narrowing the user can no longer see must never shrink the
                        // next search (the rule mail search follows, docs/search.md).
                        onSearch("")
                    } label: {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L10n.contacts_search_clear())
                }
            }
            .frame(maxWidth: showsTitle ? nil : .infinity)
            if canCreate, let onCreate {
                Button(action: onCreate) { Image(systemName: "plus") }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L10n.contacts_new())
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var list: some View {
        List {
            ForEach(contactSections(rows)) { section in
                Section {
                    ForEach(section.rows, id: \.id) { row in
                        Button { onOpen(row) } label: {
                            ContactRowView(row: row)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(
                            row.id == selectedID
                                ? Color.accentColor.opacity(0.15) : Color.clear
                        )
                    }
                } header: {
                    Text(section.letter)
                }
            }
        }
    }

    /// The two empty states are deliberately different sentences. Telling someone who has just
    /// searched "No contacts yet" reads as though theirs had vanished.
    @ViewBuilder private var emptyState: some View {
        VStack(spacing: 8) {
            if query.isEmpty {
                Text(L10n.contacts_empty()).font(.headline)
                Text(L10n.contacts_empty_body())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            } else {
                Text(L10n.contacts_no_results()).font(.headline)
            }
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// One list row: the monogram, the name, the address under it, and the merge disclosure.
struct ContactRowView: View {
    let row: ContactRow

    var body: some View {
        HStack(spacing: 12) {
            AvatarView(avatar: row.avatar)
            VStack(alignment: .leading, spacing: 1) {
                // A card may legitimately carry an address and no name. The core leaves the name
                // EMPTY rather than filling in English text a Dutch reader would be stuck with:
                // supplying the placeholder is the client's job (docs/contacts.md §2).
                Text(row.displayName.isEmpty ? L10n.contacts_no_name() : row.displayName)
                    .lineLimit(1)
                    .truncationMode(.tail)
                if !row.primaryEmail.isEmpty {
                    Text(row.primaryEmail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                // The disclosure a merged row owes the user. Only above one, every ordinary
                // contact would otherwise carry a meaningless "In 1 accounts".
                if row.accountCount > 1 {
                    Text(L10n.contacts_in_accounts(count: Int(row.accountCount)))
                        .font(.caption2)
                        .foregroundStyle(Color.accentColor)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
    }
}
