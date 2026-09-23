// One writing style, shown back in plain words (docs/ai.md, "The reveal"): what was noticed per
// language, then the name, the person's own notes, and forgetting it. The same sheet opens after a
// run and from a library row. The descriptive fields arrive in the language of the interface and the
// habits in the language of the mail; this shows both as the core wrote them.

import Foundation
import MailcalBindings
import SwiftUI

struct WritingStyleRevealView: View {
    var model: MailboxModel
    let styleId: String
    let close: () -> Void

    @State private var detail: WritingStyleDetail?
    @State private var name = ""
    @State private var notes = ""
    @State private var forgetting = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.reveal_title()).font(.headline)
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let detail {
                        ForEach(detail.languages, id: \.language) { language in
                            section(language)
                        }
                        Divider()
                        edit
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack {
                Button(L10n.writing_style_forget(), role: .destructive) { forgetting = true }
                    .disabled(detail == nil)
                Spacer()
                Button(L10n.action_cancel(), role: .cancel) { close() }
                Button(L10n.reveal_save()) { save() }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .disabled(detail == nil || trimmedName.isEmpty)
            }
        }
        .padding(16)
        #if os(macOS)
        .frame(minWidth: 520, minHeight: 520)
        #else
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        #endif
        .task { load() }
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, which
        // drops the `.cancel` button (the signature library's delete says the same).
        .alert(
            L10n.writing_style_forget_title(name: detail?.row.name ?? ""),
            isPresented: $forgetting
        ) {
            Button(L10n.writing_style_forget(), role: .destructive) {
                model.deleteWritingStyle(styleId)
                close()
            }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(L10n.writing_style_forget_message())
        }
    }

    // MARK: What was noticed, one language at a time

    private func section(_ style: LanguageStyleRow) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L10n.languageName(style.language)).font(.title3).bold()
            habits(L10n.reveal_greetings(), style.greetings)
            habits(L10n.reveal_sign_offs(), style.signOffs)
            field(L10n.reveal_signs_as(), style.signsAs)
            field(L10n.reveal_register(), style.register)
            if style.typicalWords > 0 {
                Text(L10n.reveal_length(count: Int(style.typicalWords))).font(.callout)
            }
            field(L10n.reveal_shape(), style.shape)
            field(L10n.reveal_punctuation(), style.punctuation)
            field(L10n.reveal_structure(), style.structure)
            field(L10n.reveal_moves(), style.moves)
            list(L10n.reveal_phrases(), style.phrases)
            list(L10n.reveal_avoid(), style.avoid)
        }
    }

    @ViewBuilder
    private func field(_ label: String, _ value: String) -> some View {
        if !value.isEmpty {
            labelled(label) { Text(value) }
        }
    }

    @ViewBuilder
    private func habits(_ label: String, _ habits: [HabitRow]) -> some View {
        if !habits.isEmpty {
            labelled(label) {
                ForEach(Array(habits.enumerated()), id: \.offset) { _, habit in
                    Text(writingStyleHabit(habit, locale: L10n.appLocale))
                }
            }
        }
    }

    @ViewBuilder
    private func list(_ label: String, _ items: [String]) -> some View {
        if !items.isEmpty {
            labelled(label) {
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                    Text(item)
                }
            }
        }
    }

    private func labelled(_ label: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.subheadline).bold()
            VStack(alignment: .leading, spacing: 2) { content() }
                .font(.callout)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: Name and notes

    private var edit: some View {
        VStack(alignment: .leading, spacing: 10) {
            LabeledContent(L10n.writing_style_name_label()) {
                TextField(L10n.writing_style_name_label(), text: $name)
                    .textFieldStyle(.roundedBorder)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(L10n.writing_style_notes()).font(.subheadline).bold()
                Text(L10n.writing_style_notes_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                TextEditor(text: $notes)
                    .frame(minHeight: 80)
                    .border(.quaternary)
            }
        }
    }

    private var trimmedName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func load() {
        detail = model.writingStyleDetail(styleId)
        name = detail?.row.name ?? ""
        notes = detail?.notes ?? ""
    }

    private func save() {
        model.saveWritingStyle(styleId, name: trimmedName, notes: notes)
        close()
    }
}
