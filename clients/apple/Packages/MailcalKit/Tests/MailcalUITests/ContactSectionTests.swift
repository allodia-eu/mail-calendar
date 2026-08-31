// The A–Z grouping of the contacts list (ContactSections.swift).
//
// The core owns the ordering, the section letter, and the `#` bucket, including that an accented
// letter is a letter. What this layer must not do is have an opinion of its own: a client that
// re-buckets or re-sorts introduces a second ordering that can disagree with the first, and the
// symptom is a list that looks fine and is quietly in a different order on one platform.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ContactSectionTests {

    private func row(_ name: String, section: String, accounts: UInt32 = 1) -> ContactRow {
        ContactRow(
            id: "id:\(name)",
            displayName: name,
            primaryEmail: "\(name.lowercased())@example.test",
            section: section,
            avatar: Avatar(
                initials: String(name.prefix(1)),
                light: swatch,
                dark: swatch,
                imagePath: nil
            ),
            accountCount: accounts
        )
    }

    /// Any colour: nothing here renders, and the palette is the core's to decide.
    private var swatch: Swatch {
        Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB")
    }

    @Test func rowsAreGroupedUnderTheSectionTheCoreGaveThem() {
        let sections = contactSections([
            row("Ada", section: "A"),
            row("Émile", section: "E"),
            row("Emma", section: "E"),
            row("42 Ltd", section: "#"),
        ])
        #expect(sections.map(\.letter) == ["A", "E", "#"])
        #expect(sections[1].rows.map(\.displayName) == ["Émile", "Emma"])
    }

    @Test func theOrderTheCoreGaveIsTheOrderRendered() {
        // Grouping by key would silently re-order, and `#` sorting after Z (or before A) is a
        // decision the core has already made. Flattening the sections must give back exactly the
        // input, in the input's order.
        let rows = [
            row("Ada", section: "A"),
            row("Émile", section: "E"),
            row("Emma", section: "E"),
            row("42 Ltd", section: "#"),
        ]
        #expect(contactSections(rows).flatMap(\.rows).map(\.id) == rows.map(\.id))
    }

    @Test func anEmptyListHasNoSections() {
        #expect(contactSections([]).isEmpty)
    }

    @Test func everySectionCarriesADistinctIdentity() {
        // A duplicate `id` is a silently broken `List` rather than a loud failure, so the identity
        // is a person's id rather than the letter, which holds even for input the core would
        // never produce.
        let sections = contactSections([
            row("Ada", section: "A"),
            row("Bo", section: "B"),
            row("Ann", section: "A"),
        ])
        #expect(Set(sections.map(\.id)).count == sections.count)
    }
}
