// The contact editor sheet, and the small question that precedes it when a person is filed in
// more than one account.
//
// The email and phone fields are LISTS the user adds to and removes from, and their order is the
// card's order. Every decision lives in ContactEditorModel next door; this file only draws it.

import MailcalBindings
import SwiftUI

struct ContactEditorView: View {
    @Bindable var model: ContactEditorModel
    let onSave: (Intent) -> Void
    let onCancel: () -> Void

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField(L10n.contacts_first_name(), text: $model.givenName)
                    TextField(L10n.contacts_last_name(), text: $model.surname)
                    TextField(L10n.contacts_section_organizations(), text: $model.organization)
                    TextField(L10n.contacts_section_titles(), text: $model.title)
                }
                valueSection(
                    heading: L10n.contacts_section_emails(),
                    addLabel: L10n.contacts_add_email(),
                    removeLabel: L10n.contacts_remove_email(),
                    values: $model.emails
                )
                valueSection(
                    heading: L10n.contacts_section_phones(),
                    addLabel: L10n.contacts_add_phone(),
                    removeLabel: L10n.contacts_remove_phone(),
                    values: $model.phones
                )
                // Only a create files a contact somewhere new, and only when there is a choice to
                // make: one address book is a fact, not a decision.
                if model.picksTarget {
                    Section {
                        Picker(L10n.contacts_address_book(), selection: $model.target) {
                            ForEach(model.targets) { target in
                                Text(target.label).tag(Optional(target))
                            }
                        }
                    }
                }
                if model.showsError, let error = model.error {
                    Section {
                        Text(
                            error == .empty
                                ? L10n.contacts_editor_invalid()
                                : L10n.contacts_editor_invalid_email()
                        )
                        .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle(model.isEditing ? L10n.contacts_edit() : L10n.contacts_new())
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel(), action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_save()) {
                        if let intent = model.intent() {
                            onSave(intent)
                        } else {
                            model.showsError = true
                        }
                    }
                }
            }
        }
    }

    /// One headed, repeating field: a row per value, plus the button that adds another.
    @ViewBuilder
    private func valueSection(
        heading: String,
        addLabel: String,
        removeLabel: String,
        values: Binding<[String]>
    ) -> some View {
        Section(heading) {
            // Indexed, because the row writes back by position, which is what keeps the order on
            // screen the order that is saved.
            ForEach(values.indices, id: \.self) { index in
                HStack {
                    TextField(heading, text: values[index])
                    Button {
                        values.wrappedValue.remove(at: index)
                    } label: {
                        Image(systemName: "minus.circle.fill").foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(removeLabel)
                }
            }
            Button(addLabel) { values.wrappedValue.append("") }
        }
    }
}

/// Asks which account's card to edit, when the person is filed in more than one.
///
/// Its own step rather than a picker inside the editor, because the answer decides what the form
/// is *seeded with*: a merged person's values belong to different cards, and letting the user
/// change accounts mid-edit would have to throw away what they had typed.
struct ContactCardChoiceView: View {
    let cards: [ContactCardChoice]
    let onPick: (ContactCardChoice) -> Void
    let onCancel: () -> Void

    var body: some View {
        NavigationStack {
            List {
                Section(L10n.contacts_pick_card()) {
                    ForEach(cards) { card in
                        Button(card.label) { onPick(card) }
                    }
                }
            }
            .navigationTitle(L10n.contacts_edit())
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel(), action: onCancel)
                }
            }
        }
    }
}
