// A new style shown back once (docs/ai.md rule 15), in the flow host's paged result: the pages the
// core composed, in its order, then the name and notes with Save. It counts as shown once left, by
// Save or by closing. The same view follows a run in the learning flow's host and opens from a
// library row whose reveal has not been shown.

import MailcalBindings
import SwiftUI

struct WritingStyleRevealView: View {
    var model: MailboxModel
    let styleId: String
    /// Leaves the sheet, once the core has the change it was left by.
    let close: () -> Void

    @State private var name = ""
    @State private var notes = ""
    @State private var language = 0
    @State private var session: WritingStyleSheetSession
    /// Why Save did not take.
    @State private var problem: String?

    init(model: MailboxModel, styleId: String, close: @escaping () -> Void) {
        self.model = model
        self.styleId = styleId
        self.close = close
        _session = State(initialValue: WritingStyleSheetSession(
            style: styleId, page: model.writingStyle.page))
        let reveal = model.writingStyleReveal(styleId)
        _name = State(initialValue: reveal?.name ?? "")
        _notes = State(initialValue: reveal?.notes ?? "")
    }

    var body: some View {
        // Read again whenever the core says the feature's records moved.
        let _ = model.writingStyle.version
        // Asked here, so it follows every keystroke and every language chosen: the pages read
        // both inside the host, where a change to this view's state would not reach them.
        let refusal = writingStyleNameRefusal(name: name)
        let _ = language
        Group {
            if let reveal = model.writingStyleReveal(styleId) {
                pages(reveal, refusal: refusal)
            }
        }
        .onChange(of: model.writingStyle.page?.outcome?.serial) { _, _ in
            switch session.take(model.writingStyle.page?.outcome) {
            case .close: close()
            case let .refused(said): problem = said
            case .stay, nil: break
            }
        }
    }

    private func pages(_ reveal: WritingStyleReveal, refusal: WritingStyleRefusal?) -> some View {
        let pages = reveal.pages
        return FlowPages(
            count: pages.count,
            title: { WritingStyleWords.page(pages[$0]) },
            cancel: FlowControl(title: L10n.action_close(), control: reveal.close) {
                model.changeWritingStyle(.closeReveal(style: styleId))
                close()
            },
            primary: { index in
                guard pages[index] == .name else { return nil }
                return FlowControl(
                    title: L10n.reveal_save(), control: reveal.save.held(refusal != nil)
                ) {
                    problem = nil
                    model.changeWritingStyle(.saveReveal(style: styleId, name: name, notes: notes))
                }
            },
            backTitle: L10n.wizard_back(),
            nextTitle: L10n.wizard_next(),
            progressLabel: { L10n.a11y_flow_page(page: String($0 + 1), count: pages.count) },
            paging: reveal.paging
        ) { index in
            page(pages[index], reveal, refusal: refusal)
        }
        // A swipe down is closing, held exactly as Close is.
        .interactiveDismissDisabled(!reveal.close.enabled)
    }

    @ViewBuilder
    private func page(
        _ page: WritingStyleRevealPage, _ reveal: WritingStyleReveal, refusal: WritingStyleRefusal?
    ) -> some View {
        switch page {
        case .read:
            RevealPage { shown in
                RevealFactsSection(facts: reveal.facts, shown: shown)
                Text(L10n.reveal_pages_intro())
                    .font(RevealFont.text)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 8)
                    .revealEntrance(shown, .rise, delay: 1)
            }
        case .letter, .habits, .voice, .phrases:
            perLanguage(page, reveal.languages)
        case .name:
            namePage(reveal, refusal: refusal)
        }
    }

    /// A page about one language at a time, with the control to choose it when there are several.
    private func perLanguage(
        _ page: WritingStyleRevealPage, _ languages: [WritingStyleLanguage]
    ) -> some View {
        let index = languages.indices.contains(language) ? language : 0
        return RevealPage { shown in
            RevealLanguagePicker(languages: languages, selected: $language)
            if languages.indices.contains(index) {
                section(page, languages[index], shown: shown)
                    .id(index)
            }
        }
    }

    @ViewBuilder
    private func section(
        _ page: WritingStyleRevealPage, _ language: WritingStyleLanguage, shown: Bool
    ) -> some View {
        switch page {
        case .letter:
            if let letter = language.letter {
                RevealLetterSection(letter: letter, shown: shown)
            } else {
                nothing
            }
        case .habits:
            if let habits = language.habits {
                RevealHabitsSection(habits: habits, shown: shown)
            } else {
                nothing
            }
        case .voice:
            if language.cards.isEmpty {
                nothing
            } else {
                RevealCardsSection(cards: language.cards, shown: shown)
            }
        case .phrases:
            if let phrases = language.phrases {
                RevealPhrasesSection(phrases: phrases, shown: shown)
            } else {
                nothing
            }
        case .read, .name:
            EmptyView()
        }
    }

    private var nothing: some View {
        Text(L10n.reveal_nothing_in_language())
            .font(RevealFont.text)
            .foregroundStyle(.secondary)
    }

    private func namePage(_ reveal: WritingStyleReveal, refusal: WritingStyleRefusal?) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            HeldFields(control: reveal.save) {
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(L10n.writing_style_name_label())
                            .font(RevealFont.label)
                            .foregroundStyle(.secondary)
                        TextField(L10n.writing_style_name_label(), text: $name)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityIdentifier("writingStyle.reveal.name")
                        if let refusal {
                            Text(WritingStyleWords.refusal(refusal))
                                .font(RevealFont.caption)
                                .foregroundStyle(.orange)
                        }
                    }
                    WritingStyleNotesEditor(notes: $notes)
                }
            }
            if let problem {
                Text(problem)
                    .font(RevealFont.caption)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A page whose parts play in once it is on screen.
private struct RevealPage<Content: View>: View {
    @ViewBuilder let content: (Bool) -> Content

    @State private var shown = false

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            content(shown)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear { shown = true }
    }
}

/// The person's own notes, which win over the description: a few lines, with what they are for.
struct WritingStyleNotesEditor: View {
    @Binding var notes: String

    @Environment(\.colorScheme) private var scheme
    /// A few lines of the text it holds, at whatever size the text is.
    @ScaledMetric(relativeTo: .body) private var minHeight: CGFloat = 92
    @ScaledMetric(relativeTo: .body) private var maxHeight: CGFloat = 160

    var body: some View {
        let palette = RevealPalette(scheme)
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.writing_style_notes())
                .font(RevealFont.label)
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)
            Text(L10n.writing_style_notes_hint())
                .font(RevealFont.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityHidden(true)
            TextEditor(text: $notes)
                .font(RevealFont.text)
                .scrollContentBackground(.hidden)
                .padding(4)
                .frame(minHeight: minHeight, maxHeight: maxHeight)
                .background(palette.paper, in: RoundedRectangle(cornerRadius: 7))
                .overlay(
                    RoundedRectangle(cornerRadius: 7).strokeBorder(palette.separator, lineWidth: 0.5))
                .accessibilityLabel(L10n.writing_style_notes())
                .accessibilityHint(L10n.writing_style_notes_hint())
                .accessibilityIdentifier("writingStyle.notes")
        }
    }
}
