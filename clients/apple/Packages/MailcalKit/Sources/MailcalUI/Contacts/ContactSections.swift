// The A–Z grouping of the contacts list, and the identity of an opened contact.
//
// The core decides which section each person files under (`ContactRow.section`), including that an
// accented letter is a letter, and that everything else collects in one `#` bucket rather than each
// symbol minting its own. This turns that flat ordered list into the sections a `List` renders, and
// does nothing else: no sorting, no re-bucketing, no matching. All three live in the core precisely
// so that three clients cannot disagree about them.

import MailcalBindings

/// One A–Z section of the contacts list: its letter, and the people filed under it.
struct ContactSection: Identifiable {
    let letter: String
    let rows: [ContactRow]

    /// Identified by its first person rather than by its letter. The letter is unique in any list
    /// the core actually produces, but a duplicate `id` is a silently broken `List` rather than a
    /// loud failure, and a person's id is unique by construction.
    var id: String { rows.first?.id ?? letter }
}

/// Groups the core's ordered rows into A–Z sections.
///
/// Groups **consecutive runs**, never re-buckets by key: the core hands back one flat ordered list,
/// and re-grouping it here would be a second ordering that could disagree with the first, the same
/// reason the Android list decides a header by comparing with the previous row. Order in, order out.
func contactSections(_ rows: [ContactRow]) -> [ContactSection] {
    var sections: [ContactSection] = []
    var current: [ContactRow] = []
    for row in rows {
        if let first = current.first, first.section != row.section {
            sections.append(ContactSection(letter: first.section, rows: current))
            current = []
        }
        current.append(row)
    }
    if let first = current.first {
        sections.append(ContactSection(letter: first.section, rows: current))
    }
    return sections
}

/// The contact whose detail is on screen.
///
/// A wrapper rather than the FFI record itself, so the shell can drive an iPhone push
/// (`navigationDestination(item:)`) and a macOS/iPad selection from one piece of state.
struct OpenedContact: Identifiable, Hashable {
    let detail: ContactDetail

    /// The **resolved** person's id, which need not be the row id it was opened from: merging
    /// retires ids, and the core keeps the retired ones pointing at the surviving person.
    var id: String { detail.id }

    // `navigationDestination(item:)` needs `Hashable`, and identity here is the person, not the
    // values on their card. Two reads of the same person that differ only because a sync landed
    // between them must not read as two different pushes.
    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
    func hash(into hasher: inout Hasher) { hasher.combine(id) }
}
