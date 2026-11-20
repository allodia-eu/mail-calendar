// Tone and approach as cards, and the person's phrases as chips with what they avoid struck
// through (docs/ai.md rule 15). The headings and the order are the core's.

import MailcalBindings
import SwiftUI

/// Tone and approach: one card per topic the style has something for.
struct RevealCardsSection: View {
    let cards: [WritingStyleCard]
    let shown: Bool

    var body: some View {
        VStack(spacing: 10) {
            ForEach(Array(cards.enumerated()), id: \.offset) { index, card in
                RevealCard(card: card)
                    .revealEntrance(shown, .rise, delay: 0.06 + 0.09 * Double(index))
            }
        }
    }
}

/// One card: a symbol, what it is about, the heading and the text beneath it.
private struct RevealCard: View {
    let card: WritingStyleCard

    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let palette = RevealPalette(scheme)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: symbol)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 28, height: 28)
                    .background(palette.accentFaint, in: RoundedRectangle(cornerRadius: 7))
                    .accessibilityHidden(true)
                Text(WritingStyleWords.topic(card.topic))
                    .font(RevealFont.label)
                    .foregroundStyle(.secondary)
            }
            Text(card.heading)
                .font(RevealFont.headline)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 9)
            if !card.body.isEmpty {
                Text(card.body)
                    .font(RevealFont.text)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 3)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 13)
        .padding(.bottom, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .revealCard(palette, radius: 12)
        .accessibilityElement(children: .combine)
    }

    private var symbol: String {
        switch card.topic {
        case .register: "slider.horizontal.3"
        case .structure: "list.bullet.indent"
        case .moves: "bubble.left.and.bubble.right"
        case .punctuation: "textformat"
        }
    }
}

/// The phrases the person uses, and what they avoid.
struct RevealPhrasesSection: View {
    let phrases: WritingStylePhrases
    let shown: Bool

    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let palette = RevealPalette(scheme)
        VStack(alignment: .leading, spacing: 14) {
            if !phrases.used.isEmpty {
                RecipientFlowLayout(spacing: 8) {
                    ForEach(Array(phrases.used.enumerated()), id: \.offset) { index, phrase in
                        chip(phrase)
                            .revealCard(palette, radius: 16)
                            .revealEntrance(shown, .pop, delay: 0.06 + 0.04 * Double(index))
                    }
                }
            }
            if !phrases.avoided.isEmpty {
                Text(L10n.reveal_avoid())
                    .font(RevealFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityAddTraits(.isHeader)
                RecipientFlowLayout(spacing: 8) {
                    ForEach(Array(phrases.avoided.enumerated()), id: \.offset) { index, phrase in
                        chip(phrase)
                            .strikethrough(true, color: Color.red.opacity(0.7))
                            .foregroundStyle(.secondary)
                            .overlay(
                                RoundedRectangle(cornerRadius: 16)
                                    .strokeBorder(palette.separator, lineWidth: 1))
                            .revealEntrance(shown, .pop, delay: 0.42 + 0.04 * Double(index))
                    }
                }
            }
        }
    }

    private func chip(_ phrase: String) -> some View {
        Text(phrase)
            .font(RevealFont.chip)
            .padding(.horizontal, 14)
            .padding(.vertical, 4)
            .frame(minHeight: 32)
    }
}

/// One language's sections as the detail draws them, settled and on one page: the sections the
/// record lists, in its order, each saying so for a language with nothing for it.
struct RevealLanguageSections: View {
    let language: WritingStyleLanguage
    let sections: [WritingStyleSection]

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            ForEach(Array(sections.enumerated()), id: \.offset) { _, kind in
                section(title(kind)) { content(kind) }
            }
        }
    }

    @ViewBuilder
    private func content(_ kind: WritingStyleSection) -> some View {
        switch kind {
        case .letter:
            if let letter = language.letter { RevealLetterSection(letter: letter, shown: true) } else { nothing }
        case .habits:
            if let habits = language.habits { RevealHabitsSection(habits: habits, shown: true) } else { nothing }
        case .voice:
            if language.cards.isEmpty { nothing } else { RevealCardsSection(cards: language.cards, shown: true) }
        case .phrases:
            if let phrases = language.phrases { RevealPhrasesSection(phrases: phrases, shown: true) } else { nothing }
        }
    }

    private var nothing: some View {
        Text(L10n.reveal_nothing_in_language()).foregroundStyle(.secondary)
    }

    private func title(_ kind: WritingStyleSection) -> String {
        switch kind {
        case .letter: L10n.reveal_step_letter()
        case .habits: L10n.reveal_step_habits()
        case .voice: L10n.reveal_step_voice()
        case .phrases: L10n.reveal_phrases()
        }
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline).accessibilityAddTraits(.isHeader)
            content()
        }
    }
}

/// The language a per-language page or the detail shows, chosen among the style's: drawn only for
/// two or more.
struct RevealLanguagePicker: View {
    let languages: [WritingStyleLanguage]
    @Binding var selected: Int

    var body: some View {
        if languages.count > 1 {
            Picker(L10n.a11y_reveal_language(), selection: $selected) {
                ForEach(Array(languages.enumerated()), id: \.offset) { index, language in
                    Text(WritingStyleWords.language(language.language)).tag(index)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            .accessibilityLabel(L10n.a11y_reveal_language())
        }
    }
}
