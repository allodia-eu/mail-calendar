// The reveal's first three pages: what was read, a typical reply as a miniature letter, and the
// greetings and sign-offs as bars. Each page starts hidden and plays in once it is on screen; the
// arithmetic behind the pictures is in WritingStyleReveal.swift.

import Foundation
import MailcalBindings
import SwiftUI

/// Step 1: how many messages in how many languages since when, from which account.
struct RevealReadPage: View {
    let detail: WritingStyleDetail
    /// The source account's address; `nil` when the style came from another device.
    let address: String?
    let zone: TimeZone

    @State private var shown = false
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .largeTitle) private var numeral: CGFloat = 46

    private static let languageColours: [Color] = [.accentColor, .indigo, .teal, .orange, .pink, .green]

    var body: some View {
        WizardPage(title: L10n.reveal_title()) {
            if let address {
                Text(L10n.reveal_based_on(address: address))
                    .font(WizardFont.text)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, -6)
            }
            statsRow
                .padding(.top, 18)
            languageChips
                .padding(.top, 14)
            Spacer(minLength: 16)
            Text(L10n.reveal_pages_intro())
                .font(WizardFont.text)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .wizardEntrance(shown, .rise, delay: 1)
        }
        .onAppear { shown = true }
    }

    private var codes: [String] { detail.languages.map(\.language) }

    private var since: String? {
        detail.row.oldest.map { revealSinceText($0, now: Date(), zone: zone, locale: L10n.appLocale) }
    }

    /// The row read as one sentence, when there is a date to say it with.
    @ViewBuilder
    private var statsRow: some View {
        let row = ViewThatFits(in: .horizontal) {
            stats(numeral)
            stats(numeral * 0.78)
            stats(numeral * 0.6)
        }
        if let oldest = detail.row.oldest {
            row
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(L10n.reveal_stats_a11y(
                    count: Int(detail.row.messages),
                    date: writingStyleDate(oldest, zone: zone, locale: L10n.appLocale),
                    languages: writingStyleLanguages(codes, locale: L10n.appLocale)
                ))
        } else {
            row.accessibilityElement(children: .combine)
        }
    }

    private func stats(_ size: CGFloat) -> some View {
        let font = Font.system(size: size, weight: .semibold, design: .rounded).monospacedDigit()
        return HStack(alignment: .top, spacing: 0) {
            stat(L10n.reveal_stat_messages(), first: true, delay: 0) {
                counting(Int(detail.row.messages), font: font, delay: 0)
            }
            stat(L10n.reveal_stat_languages(), first: false, delay: 0.12) {
                counting(codes.count, font: font, delay: 0.12)
            }
            if let since {
                stat(L10n.reveal_stat_since(), first: false, delay: 0.24) {
                    Text(since).font(font).tracking(-size * 0.025)
                }
            }
        }
        .fixedSize()
    }

    private func stat(
        _ label: String, first: Bool, delay: Double, @ViewBuilder value: () -> some View
    ) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(WizardFont.label).foregroundStyle(.secondary)
            value()
        }
        .padding(.leading, first ? 0 : 22)
        .padding(.trailing, 22)
        .overlay(alignment: .leading) {
            if !first {
                Rectangle().fill(WizardPalette(scheme).separator).frame(width: 0.5)
            }
        }
        .wizardEntrance(shown, .rise, delay: delay)
    }

    /// A count from nought, in a box already as wide as the final value so nothing beside it moves.
    private func counting(_ target: Int, font: Font, delay: Double) -> some View {
        Text(String(target))
            .font(font)
            .hidden()
            .overlay(alignment: .leading) {
                CountingNumber(value: shown || reduceMotion ? Double(target) : 0) { Text(String($0)) }
                    .font(font)
                    .animation(reduceMotion ? nil : WizardMotion.count(0.9, delay), value: shown)
            }
    }

    private var languageChips: some View {
        let palette = WizardPalette(scheme)
        return RecipientFlowLayout(spacing: 8) {
            ForEach(Array(codes.enumerated()), id: \.offset) { index, code in
                HStack(spacing: 8) {
                    Circle()
                        .fill(Self.languageColours[index % Self.languageColours.count])
                        .frame(width: 8, height: 8)
                    Text(L10n.languageName(code)).font(WizardFont.text)
                }
                .padding(.leading, 10)
                .padding(.trailing, 12)
                .frame(minHeight: 28)
                .background(palette.fill, in: Capsule())
                .wizardEntrance(shown, .pop, delay: 0.76 + 0.08 * Double(index))
            }
        }
        // The row above already names the languages.
        .accessibilityHidden(true)
    }
}

/// Step 2: a typical reply as a miniature letter, beside its length, shape and punctuation.
struct RevealLetterPage: View {
    let style: LanguageStyleRow
    let underTopBar: Bool

    @State private var shown = false
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .title) private var greetingSize: CGFloat = 28
    @ScaledMetric(relativeTo: .body) private var signSize: CGFloat = 16
    @ScaledMetric(relativeTo: .title) private var lengthSize: CGFloat = 32

    var body: some View {
        WizardPage(title: L10n.reveal_step_letter(), underTopBar: underTopBar) {
            #if os(macOS)
            HStack(alignment: .top, spacing: 22) {
                letterColumn
                facts.frame(width: 215, alignment: .leading)
            }
            #else
            letterColumn
            facts.padding(.top, 2)
            #endif
        }
        .onAppear { shown = true }
    }

    private var lines: [[Double]] {
        revealLetterLines(words: style.typicalWords, paragraphs: style.typicalParagraphs)
    }

    private var letterColumn: some View {
        VStack(alignment: .leading, spacing: 8) {
            letter
            Text(L10n.reveal_letter_caption())
                .font(WizardFont.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 2)
        }
    }

    private var letter: some View {
        let palette = WizardPalette(scheme)
        let paragraphs = lines
        let starts = paragraphs.reduce(into: [0]) { $0.append($0[$0.count - 1] + $1.count) }
        let serif = Font.system(size: signSize, design: .serif)
        return VStack(alignment: .leading, spacing: 0) {
            if let greeting = style.greetings.first {
                RevealRunsText(
                    runs: revealRuns(greeting.text),
                    font: .system(size: greetingSize, weight: .medium, design: .serif)
                )
                .padding(.bottom, 12)
                .wizardEntrance(shown, .rise, delay: 0.04)
            }
            VStack(alignment: .leading, spacing: 12) {
                ForEach(Array(paragraphs.enumerated()), id: \.offset) { paragraph, widths in
                    VStack(alignment: .leading, spacing: 7) {
                        ForEach(Array(widths.enumerated()), id: \.offset) { line, width in
                            GrowingBar(fraction: shown || reduceMotion ? width : 0, cornerRadius: 3.5)
                                .fill(palette.ink)
                                .frame(height: 7)
                                .animation(
                                    reduceMotion ? nil : WizardMotion.grow(0.2 + 0.06 * Double(starts[paragraph] + line)),
                                    value: shown
                                )
                        }
                    }
                }
            }
            .accessibilityHidden(true)
            if style.signOffs.first != nil || !style.signsAs.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    if let signOff = style.signOffs.first {
                        RevealRunsText(runs: revealRuns(signOff.text), font: serif)
                    }
                    if !style.signsAs.isEmpty {
                        Text(style.signsAs).font(serif)
                    }
                }
                .padding(.top, 22)
                .wizardEntrance(shown, .rise, delay: 0.28 + 0.06 * Double(starts[starts.count - 1]))
            }
        }
        .padding(.top, 16)
        .padding(.horizontal, 18)
        .padding(.bottom, 18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .wizardCard(palette, radius: 12)
        .shadow(color: palette.shadow, radius: 14, y: 10)
        .accessibilityElement(children: .combine)
    }

    private var facts: some View {
        VStack(alignment: .leading, spacing: 16) {
            if style.typicalWords > 0 {
                CountingNumber(value: shown || reduceMotion ? Double(style.typicalWords) : 0) { length($0) }
                    .animation(reduceMotion ? nil : WizardMotion.count(0.7, 0.12), value: shown)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(L10n.reveal_length(count: Int(style.typicalWords)))
                    .wizardEntrance(shown, .rise, delay: 0.12)
            }
            fact(L10n.reveal_shape(), style.shape, delay: 0.24)
            fact(L10n.reveal_punctuation(), style.punctuation, delay: 0.36)
        }
        .padding(.top, 4)
    }

    /// "Usually about 70 words", the number drawn large in the sentence the catalog words.
    private func length(_ count: Int) -> Text {
        var sentence = AttributedString(L10n.reveal_length(count: count))
        sentence.foregroundColor = Color.secondary
        sentence.font = WizardFont.text
        if let number = sentence.range(of: String(count)) {
            sentence[number].font = .system(size: lengthSize, weight: .semibold, design: .rounded).monospacedDigit()
            sentence[number].foregroundColor = Color.primary
        }
        return Text(sentence)
    }

    @ViewBuilder
    private func fact(_ label: String, _ value: String, delay: Double) -> some View {
        if !value.isEmpty {
            VStack(alignment: .leading, spacing: 3) {
                Text(label).font(WizardFont.label).foregroundStyle(.secondary)
                Text(value)
                    .font(WizardFont.text)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .accessibilityElement(children: .combine)
            .wizardEntrance(shown, .rise, delay: delay)
        }
    }
}

/// Step 3: greetings and sign-offs, each over a bar as long as its part of its list.
struct RevealHabitsPage: View {
    let style: LanguageStyleRow
    let underTopBar: Bool

    @State private var shown = false
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .title3) private var signsAsSize: CGFloat = 20

    var body: some View {
        WizardPage(title: L10n.reveal_step_habits(), underTopBar: underTopBar) {
            VStack(alignment: .leading, spacing: 18) {
                group(L10n.reveal_greetings(), style.greetings, start: 0.08)
                group(L10n.reveal_sign_offs(), style.signOffs, start: 0.32)
                if !style.signsAs.isEmpty {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(L10n.reveal_signs_as()).font(WizardFont.label).foregroundStyle(.secondary)
                        Text(style.signsAs).font(.system(size: signsAsSize, weight: .medium, design: .serif))
                    }
                    .padding(.top, 2)
                    .accessibilityElement(children: .combine)
                    .wizardEntrance(shown, .rise, delay: 0.56)
                }
            }
        }
        .onAppear { shown = true }
    }

    @ViewBuilder
    private func group(_ label: String, _ habits: [HabitRow], start: Double) -> some View {
        if !habits.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(label)
                    .font(WizardFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityAddTraits(.isHeader)
                VStack(spacing: 5) {
                    ForEach(Array(habits.enumerated()), id: \.offset) { index, habit in
                        row(habit, top: index == 0, delay: start + 0.08 * Double(index))
                    }
                }
            }
        }
    }

    private func row(_ habit: HabitRow, top: Bool, delay: Double) -> some View {
        let palette = WizardPalette(scheme)
        return HStack(spacing: 12) {
            RevealRunsText(runs: revealRuns(habit.text), font: WizardFont.habit)
            Spacer(minLength: 8)
            Text(revealFrequencyText(habit.frequency))
                .font(WizardFont.small)
                .foregroundStyle(.secondary)
                .fixedSize()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
        .frame(minHeight: 34)
        .background {
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 9).fill(palette.track)
                GrowingBar(fraction: shown || reduceMotion ? Double(habit.relative) / 100 : 0, cornerRadius: 9)
                    .fill(top ? palette.accentSoft : palette.fillStrong)
                    .animation(reduceMotion ? nil : WizardMotion.grow(delay), value: shown)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A greeting or sign-off, a placeholder such as `[Name]` drawn as a small pill in the accent.
struct RevealRunsText: View {
    let runs: [RevealRun]
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
            .font(WizardFont.pill)
            .foregroundStyle(Color.accentColor)
            .padding(.horizontal, 7)
            .frame(minHeight: 18)
            .background(WizardPalette(scheme).accentFaint, in: Capsule())
            .overlay(Capsule().strokeBorder(Color.accentColor.opacity(0.4), lineWidth: 0.5))
            .padding(.horizontal, 1)
    }
}
