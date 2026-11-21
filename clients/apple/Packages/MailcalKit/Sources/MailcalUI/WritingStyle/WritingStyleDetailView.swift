// An existing style's detail (docs/ai.md rule 16): what it was learned from, its name and notes,
// the reveal's sections on one page, and Forget, asked first. Every control is the core's: the
// name's Save held while `writing_style_name_refusal` refuses it, the notes sent on Return and on
// leaving the field, Forget running until the library is written.

import MailcalBindings
import SwiftUI

struct WritingStyleDetailView: View {
    var model: MailboxModel
    let styleId: String
    let close: () -> Void

    @State private var name = ""
    @State private var loaded = false
    @State private var language = 0
    @State private var forgetting: WritingStyleDetail?
    @State private var session: WritingStyleSheetSession
    @State private var problem: String?

    init(model: MailboxModel, styleId: String, close: @escaping () -> Void) {
        self.model = model
        self.styleId = styleId
        self.close = close
        _session = State(initialValue: WritingStyleSheetSession(
            style: styleId, page: model.writingStyle.page))
    }

    var body: some View {
        let _ = model.writingStyle.version
        let detail = model.writingStyleDetail(styleId)
        chrome(title: detail?.name ?? "") {
            if let detail {
                content(detail)
                    .onAppear {
                        guard !loaded else { return }
                        loaded = true
                        name = detail.name
                    }
            }
        }
        .onChange(of: model.writingStyle.page?.outcome?.serial) { _, _ in
            switch session.take(model.writingStyle.page?.outcome) {
            case .close: close()
            case let .refused(said):
                problem = said
                AccessibilityNotification.Announcement(said).post()
            case .stay, nil: break
            }
        }
        .confirmAlert(
            $forgetting, action: L10n.writing_style_forget(),
            title: { L10n.writing_style_forget_title(name: $0.name) },
            message: { WritingStyleWords.forgetMessage($0.draftingAccounts) },
            confirm: { _ in
                problem = nil
                model.changeWritingStyle(.forget(style: styleId))
            }
        )
    }

    /// A sheet: on macOS its title over the content and Done at the foot; on iOS a navigation bar
    /// with Done at the confirmation placement.
    @ViewBuilder
    private func chrome(title: String, @ViewBuilder content: () -> some View) -> some View {
        #if os(macOS)
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Text(title).font(.title.weight(.semibold)).accessibilityAddTraits(.isHeader)
                    content()
                }
                .padding(24)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            Divider()
            HStack {
                Spacer()
                Button(L10n.action_done(), action: close).keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .frame(minWidth: 520, idealWidth: 600, minHeight: 480, idealHeight: 640)
        #else
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) { content() }
                    .padding(18)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_done(), action: close)
                }
            }
        }
        #endif
    }

    @ViewBuilder
    private func content(_ detail: WritingStyleDetail) -> some View {
        RevealFactsSection(facts: detail.facts, shown: true)
        Divider()
        nameRow(detail)
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.writing_style_notes()).font(RevealFont.label).foregroundStyle(.secondary)
            Text(L10n.writing_style_notes_hint())
                .font(RevealFont.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ControlTextField(
                title: L10n.writing_style_notes(), text: detail.notes,
                control: detail.notesControl, axis: .vertical, maxWidth: .infinity
            ) { notes in
                model.changeWritingStyle(.setNotes(style: styleId, notes: notes))
            }
            .accessibilityIdentifier("writingStyle.detail.notes")
        }
        if let problem {
            Text(problem)
                .font(.callout)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        }
        Divider()
        RevealLanguagePicker(languages: detail.languages, selected: $language)
        if detail.languages.indices.contains(language) {
            RevealLanguageSections(
                language: detail.languages[language], sections: detail.sections)
        }
        Divider()
        ControlButton(
            title: L10n.writing_style_forget(), role: .destructive, control: detail.forget
        ) {
            forgetting = detail
        }
        .accessibilityIdentifier("writingStyle.detail.forget")
    }

    private func nameRow(_ detail: WritingStyleDetail) -> some View {
        let refusal = writingStyleNameRefusal(name: name)
        return VStack(alignment: .leading, spacing: 6) {
            Text(L10n.writing_style_name_label()).font(RevealFont.label).foregroundStyle(.secondary)
            HStack(spacing: 10) {
                HeldFields(control: detail.rename) {
                    TextField(L10n.writing_style_name_label(), text: $name)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 320)
                        .accessibilityIdentifier("writingStyle.detail.name")
                }
                ControlButton(
                    title: L10n.writing_style_rename(), control: detail.rename,
                    ready: refusal == nil && name != detail.name
                ) {
                    problem = nil
                    model.changeWritingStyle(.rename(style: styleId, name: name))
                }
                .buttonStyle(.bordered)
            }
            if let refusal {
                Text(WritingStyleWords.refusal(refusal))
                    .font(RevealFont.caption)
                    .foregroundStyle(.orange)
            }
        }
    }
}
