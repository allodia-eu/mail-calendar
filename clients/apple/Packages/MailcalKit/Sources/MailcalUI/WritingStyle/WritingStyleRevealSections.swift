// What a style is shown back as (docs/ai.md rules 15 and 16): what was read, a typical reply as a
// miniature letter, and the greetings and sign-offs as bars. The reveal plays each in once its page
// is on screen; the detail draws them settled. Every number and width is the core's.

import MailcalBindings
import SwiftUI

/// What was read: how many messages, in how many languages, since when, from which address.
struct RevealFactsSection: View {
    let facts: WritingStyleFacts
    let shown: Bool

    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .largeTitle) private var numeral: CGFloat = 46

    private static let languageColours: [Color] = [.accentColor, .indigo, .teal, .orange, .pink, .green]

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(L10n.reveal_based_on(address: facts.learnedFrom))
                .font(RevealFont.text)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            stats
            languageChips
        }
    }

    private var since: String? {
        facts.since.map { WritingStyleWords.date($0) }
    }

    @ViewBuilder private var stats: some View {
        let row = ViewThatFits(in: .horizontal) {
            statsRow(numeral)
            statsRow(numeral * 0.78)
            statsRow(numeral * 0.6)
            // At the largest text sizes the labels alone outgrow a phone's width.
            statsColumn(numeral * 0.6)
        }
        if let since {
            row
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(L10n.reveal_stats_a11y(
                    count: Int(facts.messages), date: since,
                    languages: WritingStyleWords.languages(facts.languages)))
        } else {
            row.accessibilityElement(children: .combine)
        }
    }

    private func statFont(_ size: CGFloat) -> Font {
        Font.system(size: size, weight: .semibold, design: .rounded).monospacedDigit()
    }

    private func statsRow(_ size: CGFloat) -> some View {
        let font = statFont(size)
        return HStack(alignment: .top, spacing: 0) {
            stat(L10n.reveal_stat_messages(), ruled: false, delay: 0) {
                counting(Int(facts.messages), font: font, delay: 0)
            }
            stat(L10n.reveal_stat_languages(), ruled: true, delay: 0.12) {
                counting(facts.languages.count, font: font, delay: 0.12)
            }
            if let since {
                stat(L10n.reveal_stat_since(), ruled: true, delay: 0.24) {
                    Text(since).font(font).tracking(-size * 0.025)
                }
            }
        }
        .fixedSize()
    }

    /// The same figures one under another, the date free to wrap.
    private func statsColumn(_ size: CGFloat) -> some View {
        let font = statFont(size)
        return VStack(alignment: .leading, spacing: 12) {
            stat(L10n.reveal_stat_messages(), ruled: false, delay: 0) {
                counting(Int(facts.messages), font: font, delay: 0)
            }
            stat(L10n.reveal_stat_languages(), ruled: false, delay: 0.12) {
                counting(facts.languages.count, font: font, delay: 0.12)
            }
            if let since {
                stat(L10n.reveal_stat_since(), ruled: false, delay: 0.24) {
                    Text(since).font(font).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    /// One figure under its label; `ruled` draws the rule that parts it from the one before.
    private func stat(
        _ label: String, ruled: Bool, delay: Double, @ViewBuilder value: () -> some View
    ) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(RevealFont.label).foregroundStyle(.secondary)
            value()
        }
        .padding(.leading, ruled ? 22 : 0)
        .padding(.trailing, 22)
        .overlay(alignment: .leading) {
            if ruled {
                Rectangle().fill(RevealPalette(scheme).separator).frame(width: 0.5)
            }
        }
        .revealEntrance(shown, .rise, delay: delay)
    }

    /// A count from nought, in a box already as wide as the final value so nothing beside it moves.
    private func counting(_ target: Int, font: Font, delay: Double) -> some View {
        Text(String(target))
            .font(font)
            .hidden()
            .overlay(alignment: .leading) {
                CountingNumber(value: shown || reduceMotion ? Double(target) : 0) { Text(String($0)) }
                    .font(font)
                    .animation(reduceMotion ? nil : RevealMotion.count(0.9, delay), value: shown)
            }
    }

    private var languageChips: some View {
        let palette = RevealPalette(scheme)
        return RecipientFlowLayout(spacing: 8) {
            ForEach(Array(facts.languages.enumerated()), id: \.offset) { index, code in
                HStack(spacing: 8) {
                    Circle()
                        .fill(Self.languageColours[index % Self.languageColours.count])
                        .frame(width: 8, height: 8)
                    Text(WritingStyleWords.language(code)).font(RevealFont.text)
                }
                .padding(.leading, 10)
                .padding(.trailing, 12)
                .frame(minHeight: 28)
                .background(palette.fill, in: Capsule())
                .revealEntrance(shown, .pop, delay: 0.76 + 0.08 * Double(index))
            }
        }
        // The row above already names the languages.
        .accessibilityHidden(true)
    }
}

/// A typical reply as a miniature letter, beside its length and shape.
struct RevealLetterSection: View {
    let letter: WritingStyleLetter
    let shown: Bool

    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var hSize
    #endif
    @ScaledMetric(relativeTo: .title) private var greetingSize: CGFloat = 28
    @ScaledMetric(relativeTo: .body) private var signSize: CGFloat = 16
    @ScaledMetric(relativeTo: .title) private var lengthSize: CGFloat = 32

    var body: some View {
        #if os(macOS)
        HStack(alignment: .top, spacing: 22) {
            letterColumn
            facts.frame(width: 215, alignment: .leading)
        }
        #else
        if hSize == .regular {
            HStack(alignment: .top, spacing: 22) {
                letterColumn
                facts.frame(width: 240, alignment: .leading)
            }
        } else {
            VStack(alignment: .leading, spacing: 16) {
                letterColumn
                facts
            }
        }
        #endif
    }

    private var letterColumn: some View {
        VStack(alignment: .leading, spacing: 8) {
            card
            Text(L10n.reveal_letter_caption())
                .font(RevealFont.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 2)
        }
    }

    private var card: some View {
        let palette = RevealPalette(scheme)
        let paragraphs = letter.paragraphs.map(\.lines)
        let starts = paragraphs.reduce(into: [0]) { $0.append($0[$0.count - 1] + $1.count) }
        let serif = Font.system(size: signSize, design: .serif)
        return VStack(alignment: .leading, spacing: 0) {
            if !letter.opening.isEmpty {
                RevealRunsText(
                    runs: letter.opening,
                    font: .system(size: greetingSize, weight: .medium, design: .serif)
                )
                .padding(.bottom, 12)
                .revealEntrance(shown, .rise, delay: 0.04)
            }
            VStack(alignment: .leading, spacing: 12) {
                ForEach(Array(paragraphs.enumerated()), id: \.offset) { paragraph, widths in
                    VStack(alignment: .leading, spacing: 7) {
                        ForEach(Array(widths.enumerated()), id: \.offset) { line, width in
                            GrowingBar(fraction: shown || reduceMotion ? width : 0, cornerRadius: 3.5)
                                .fill(palette.ink)
                                .frame(height: 7)
                                .animation(
                                    reduceMotion
                                        ? nil
                                        : RevealMotion.grow(0.2 + 0.06 * Double(starts[paragraph] + line)),
                                    value: shown
                                )
                        }
                    }
                }
            }
            .accessibilityHidden(true)
            if !letter.closing.isEmpty || !letter.signsAs.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    if !letter.closing.isEmpty {
                        RevealRunsText(runs: letter.closing, font: serif)
                    }
                    if !letter.signsAs.isEmpty {
                        Text(letter.signsAs).font(serif)
                    }
                }
                .padding(.top, 22)
                .revealEntrance(shown, .rise, delay: 0.28 + 0.06 * Double(starts[starts.count - 1]))
            }
        }
        .padding(.top, 16)
        .padding(.horizontal, 18)
        .padding(.bottom, 18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .revealCard(palette, radius: 12)
        .shadow(color: palette.shadow, radius: 14, y: 10)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Self.spoken(letter))
    }

    /// The letter as VoiceOver reads it, one element in the contract's order: how it opens, its
    /// length, its paragraphs, how it closes and the name, each part only when the record has it.
    /// A greeting's or a sign-off's own comma is left out, since the parts are joined by one.
    static func spoken(_ letter: WritingStyleLetter) -> String {
        func words(_ spans: [WritingStyleSpan]) -> String {
            spans.map(\.text).joined()
                .trimmingCharacters(in: .whitespaces.union(CharacterSet(charactersIn: ",")))
        }
        var parts: [String] = []
        if !letter.opening.isEmpty { parts.append(L10n.a11y_letter_opening(greeting: words(letter.opening))) }
        if letter.words > 0 { parts.append(L10n.a11y_letter_words(count: Int(letter.words))) }
        if letter.paragraphCount > 0 {
            parts.append(L10n.a11y_letter_paragraphs(count: Int(letter.paragraphCount)))
        }
        if !letter.closing.isEmpty { parts.append(L10n.a11y_letter_closing(sign_off: words(letter.closing))) }
        if !letter.signsAs.isEmpty { parts.append(L10n.a11y_letter_name(name: letter.signsAs)) }
        return parts.joined(separator: ", ")
    }

    private var facts: some View {
        VStack(alignment: .leading, spacing: 16) {
            if letter.words > 0 {
                CountingNumber(value: shown || reduceMotion ? Double(letter.words) : 0) { length($0) }
                    .animation(reduceMotion ? nil : RevealMotion.count(0.7, 0.12), value: shown)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(L10n.reveal_length(count: Int(letter.words)))
                    .revealEntrance(shown, .rise, delay: 0.12)
            }
            if !letter.shape.isEmpty {
                VStack(alignment: .leading, spacing: 3) {
                    Text(L10n.reveal_shape()).font(RevealFont.label).foregroundStyle(.secondary)
                    Text(letter.shape)
                        .font(RevealFont.text)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
                .revealEntrance(shown, .rise, delay: 0.24)
            }
        }
        .padding(.top, 4)
    }

    /// "Usually about 70 words", the number drawn large inside the sentence the catalog words.
    private func length(_ count: Int) -> Text {
        var sentence = AttributedString(L10n.reveal_length(count: count))
        sentence.foregroundColor = Color.secondary
        sentence.font = RevealFont.text
        if let number = sentence.range(of: String(count)) {
            sentence[number].font =
                .system(size: lengthSize, weight: .semibold, design: .rounded).monospacedDigit()
            sentence[number].foregroundColor = Color.primary
        }
        return Text(sentence)
    }
}

/// Greetings and sign-offs, each over a bar as long as its part of its list.
struct RevealHabitsSection: View {
    let habits: WritingStyleHabits
    let shown: Bool

    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .title3) private var signsAsSize: CGFloat = 20

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            group(L10n.reveal_greetings(), habits.greetings, start: 0.08)
            group(L10n.reveal_sign_offs(), habits.signOffs, start: 0.32)
            if !habits.signsAs.isEmpty {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(L10n.reveal_signs_as()).font(RevealFont.label).foregroundStyle(.secondary)
                    Text(habits.signsAs)
                        .font(.system(size: signsAsSize, weight: .medium, design: .serif))
                }
                .padding(.top, 2)
                .accessibilityElement(children: .combine)
                .revealEntrance(shown, .rise, delay: 0.56)
            }
        }
    }

    @ViewBuilder
    private func group(_ label: String, _ list: [WritingStyleHabit], start: Double) -> some View {
        if !list.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(label)
                    .font(RevealFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityAddTraits(.isHeader)
                VStack(spacing: 5) {
                    ForEach(Array(list.enumerated()), id: \.offset) { index, habit in
                        row(habit, top: index == 0, delay: start + 0.08 * Double(index))
                    }
                }
            }
        }
    }

    private func row(_ habit: WritingStyleHabit, top: Bool, delay: Double) -> some View {
        let palette = RevealPalette(scheme)
        let runs = RevealRunsText(runs: habit.runs, font: RevealFont.habit)
        let frequency = Text(WritingStyleWords.frequency(habit.frequency))
            .font(RevealFont.small)
            .foregroundStyle(.secondary)
            .fixedSize()
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 12) {
                runs
                Spacer(minLength: 8)
                frequency
            }
            // At the largest text sizes the two no longer share a line without breaking words.
            VStack(alignment: .leading, spacing: 2) {
                runs
                frequency
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
        .frame(minHeight: 34)
        .background {
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 9).fill(palette.track)
                GrowingBar(
                    fraction: shown || reduceMotion ? Double(habit.relative) / 100 : 0,
                    cornerRadius: 9
                )
                .fill(top ? palette.accentSoft : palette.fillStrong)
                .animation(reduceMotion ? nil : RevealMotion.grow(delay), value: shown)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A greeting or sign-off, a placeholder such as `[Name]` drawn as a small pill in the accent.
struct RevealRunsText: View {
    let runs: [WritingStyleSpan]
    let font: Font

    @Environment(\.colorScheme) private var scheme

    var body: some View {
        if runs.allSatisfy({ !$0.placeholder }) {
            Text(runs.map(\.text).joined()).font(font)
        } else {
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                ForEach(Array(runs.enumerated()), id: \.offset) { _, run in
                    if run.placeholder {
                        pill(run.text)
                    } else {
                        Text(run.text).font(font)
                    }
                }
            }
        }
    }

    private func pill(_ text: String) -> some View {
        Text(text)
            .font(RevealFont.pill)
            .foregroundStyle(Color.accentColor)
            .padding(.horizontal, 7)
            .frame(minHeight: 18)
            .background(RevealPalette(scheme).accentFaint, in: Capsule())
            .overlay(Capsule().strokeBorder(Color.accentColor.opacity(0.4), lineWidth: 0.5))
            .padding(.horizontal, 1)
    }
}
