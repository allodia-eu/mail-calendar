// One writing style, shown back in plain words (docs/ai.md, "Learning" step 5): what was read, four
// pages about one language at a time, then the name, the person's own notes, and forgetting it. The
// same sheet opens after a run and from a library row. The descriptive fields arrive in the
// language of the interface and the habits in the language of the mail; this shows both as the
// core wrote them.

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
    @State private var pager = WizardPager(count: RevealStep.allCases.count)
    @State private var language = 0
    @Environment(\.colorScheme) private var scheme

    private var step: RevealStep { RevealStep.allCases[pager.index] }
    private var languages: [LanguageStyleRow] { detail?.languages ?? [] }
    /// A single language has nothing to choose between, so the control is drawn only for two or more.
    private var choosesLanguage: Bool { languages.count > 1 }

    var body: some View {
        WizardSheet(
            pager: $pager,
            cancel: WizardAction(title: L10n.action_cancel(), role: .cancel, prominent: false, action: close),
            showsBack: !pager.isFirst,
            primary: primary,
            allowsJump: detail != nil,
            principalVisible: choosesLanguage && step.isPerLanguage
        ) {
            languagePicker
        } page: { index in
            page(RevealStep.allCases[index])
        }
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

    private var primary: WizardPrimary {
        guard pager.isLast else { return .next(enabled: detail != nil) }
        return .action(WizardAction(
            title: L10n.reveal_save(), enabled: detail != nil && !trimmedName.isEmpty, action: save
        ))
    }

    private var languagePicker: some View {
        Picker(L10n.a11y_reveal_language(), selection: $language) {
            ForEach(Array(languages.enumerated()), id: \.offset) { index, style in
                Text(L10n.languageName(style.language)).tag(index)
            }
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .fixedSize()
        .accessibilityLabel(L10n.a11y_reveal_language())
    }

    @ViewBuilder
    private func page(_ step: RevealStep) -> some View {
        if let detail {
            let style = languages.indices.contains(language) ? languages[language] : nil
            switch step {
            case .read:
                RevealReadPage(detail: detail, address: sourceAddress, zone: displayZone)
            case .letter:
                if let style {
                    RevealLetterPage(style: style, underTopBar: choosesLanguage).id(language)
                }
            case .habits:
                if let style {
                    RevealHabitsPage(style: style, underTopBar: choosesLanguage).id(language)
                }
            case .voice:
                if let style {
                    RevealVoicePage(style: style, underTopBar: choosesLanguage).id(language)
                }
            case .phrases:
                if let style {
                    RevealPhrasesPage(style: style, underTopBar: choosesLanguage).id(language)
                }
            case .name:
                namePage
            }
        }
    }

    // MARK: Name and notes

    private var namePage: some View {
        let palette = WizardPalette(scheme)
        return WizardPage(title: L10n.reveal_step_name()) {
            VStack(alignment: .leading, spacing: 5) {
                Text(L10n.writing_style_name_label())
                    .font(WizardFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                TextField(L10n.writing_style_name_label(), text: $name)
                    .textFieldStyle(.plain)
                    .font(WizardFont.text)
                    .padding(.horizontal, 9)
                    .padding(.vertical, fieldPadding)
                    .background(palette.paper, in: RoundedRectangle(cornerRadius: 7))
                    .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(palette.separator, lineWidth: 0.5))
            }
            VStack(alignment: .leading, spacing: 5) {
                Text(L10n.writing_style_notes())
                    .font(WizardFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text(L10n.writing_style_notes_hint())
                    .font(WizardFont.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityHidden(true)
                TextEditor(text: $notes)
                    .font(WizardFont.text)
                    .scrollContentBackground(.hidden)
                    .padding(4)
                    .frame(minHeight: 92, maxHeight: 140)
                    .background(palette.paper, in: RoundedRectangle(cornerRadius: 7))
                    .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(palette.separator, lineWidth: 0.5))
                    .accessibilityLabel(L10n.writing_style_notes())
                    .accessibilityHint(L10n.writing_style_notes_hint())
            }
            .padding(.top, 2)
            Spacer(minLength: 16)
            Button(L10n.writing_style_forget(), role: .destructive) { forgetting = true }
                .buttonStyle(.borderless)
                .foregroundStyle(.red)
                .disabled(detail == nil)
        }
    }

    private var fieldPadding: CGFloat {
        #if os(macOS)
        6
        #else
        10
        #endif
    }

    /// The address of the account the style was learned from, while that account is still here.
    private var sourceAddress: String? {
        guard let source = detail?.row.sourceAccount, !source.isEmpty else { return nil }
        return model.writingStyles.accounts.first { $0.accountId == source }?.email
    }

    private var trimmedName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var displayZone: TimeZone {
        TimeZone(identifier: model.activeZone) ?? .current
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
