// The reveal's fourth and fifth pages: tone and approach as three cards, and the person's phrases
// as chips, with what they avoid struck through.

import CoreGraphics
import MailcalBindings
import SwiftUI

/// Step 4: register, structure and how the person declines and chases, one card each.
struct RevealVoicePage: View {
    let style: LanguageStyleRow
    let underTopBar: Bool

    @State private var shown = false

    var body: some View {
        WizardPage(title: L10n.reveal_step_voice(), underTopBar: underTopBar) {
            VStack(spacing: 10) {
                ForEach(Array(cards.enumerated()), id: \.offset) { index, card in
                    card.wizardEntrance(shown, .rise, delay: 0.06 + 0.09 * Double(index))
                }
            }
        }
        .onAppear { shown = true }
    }

    private var cards: [RevealCard] {
        [
            RevealCard(
                symbol: "slider.horizontal.3", label: L10n.reveal_register(),
                text: revealCardText(headline: style.registerHeadline, description: style.register)
            ),
            RevealCard(
                symbol: "arrowshape.turn.up.left", label: L10n.reveal_structure(),
                text: revealCardText(headline: style.structureHeadline, description: style.structure)
            ),
            RevealCard(
                symbol: "bubble.left.and.bubble.right", label: L10n.reveal_moves(),
                text: revealCardText(headline: style.movesHeadline, description: style.moves)
            ),
        ].filter { !$0.text.headline.isEmpty || !$0.text.body.isEmpty }
    }
}

/// One card: a symbol, what it is about, a heading, and two lines of the description that open to
/// the rest.
struct RevealCard: View {
    let symbol: String
    let label: String
    let text: (headline: String, body: String)

    @State private var expanded = false
    @State private var fullHeight: CGFloat = 0
    @State private var clampedHeight: CGFloat = 0
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let palette = WizardPalette(scheme)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: symbol)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 28, height: 28)
                    .background(palette.accentFaint, in: RoundedRectangle(cornerRadius: 7))
                    .accessibilityHidden(true)
                Text(label).font(WizardFont.label).foregroundStyle(.secondary)
                Spacer(minLength: 8)
                if expanded || fullHeight > clampedHeight + 1 {
                    more
                }
            }
            if !text.headline.isEmpty {
                Text(text.headline)
                    .font(WizardFont.headline)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 9)
                    .padding(.bottom, 3)
            }
            if !text.body.isEmpty {
                description
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 13)
        .padding(.bottom, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .wizardCard(palette, radius: 12)
    }

    /// Two lines, measured against the whole text laid out unseen behind them, which is how the
    /// card knows whether there is more to open.
    private var description: some View {
        Text(text.body)
            .font(WizardFont.text)
            .lineLimit(expanded ? nil : 2)
            .fixedSize(horizontal: false, vertical: true)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height in
                if !expanded { clampedHeight = height }
            }
            .background(alignment: .top) {
                Text(text.body)
                    .font(WizardFont.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .hidden()
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { fullHeight = $0 }
            }
    }

    private var more: some View {
        Button {
            withAnimation(reduceMotion ? nil : .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.3)) {
                expanded.toggle()
            }
        } label: {
            HStack(spacing: 4) {
                Text(expanded ? L10n.reveal_card_less() : L10n.reveal_card_more())
                Image(systemName: "chevron.down")
                    .font(.system(size: 9, weight: .semibold))
                    .rotationEffect(.degrees(expanded ? 180 : 0))
            }
            .font(WizardFont.small.weight(.medium))
            .foregroundStyle(Color.accentColor)
            .padding(.vertical, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Step 5: the person's phrases, and what they avoid.
struct RevealPhrasesPage: View {
    let style: LanguageStyleRow
    let underTopBar: Bool

    @State private var shown = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let palette = WizardPalette(scheme)
        WizardPage(title: L10n.reveal_phrases(), underTopBar: underTopBar) {
            if !style.phrases.isEmpty {
                CentredFlowLayout(spacing: 8) {
                    ForEach(Array(style.phrases.enumerated()), id: \.offset) { index, phrase in
                        chip(phrase)
                            .wizardCard(palette, radius: 16)
                            .wizardEntrance(shown, .pop, delay: 0.06 + 0.04 * Double(index))
                    }
                }
                .padding(.top, 10)
                .padding(.horizontal, 4)
            }
            if !style.avoid.isEmpty {
                Text(L10n.reveal_avoid())
                    .font(WizardFont.label)
                    .foregroundStyle(.secondary)
                    .accessibilityAddTraits(.isHeader)
                    .padding(.top, 14)
                RecipientFlowLayout(spacing: 8) {
                    ForEach(Array(style.avoid.enumerated()), id: \.offset) { index, phrase in
                        chip(phrase)
                            .strikethrough(true, color: Color.red.opacity(0.7))
                            .foregroundStyle(.secondary)
                            .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(palette.separator, lineWidth: 1))
                            .wizardEntrance(shown, .pop, delay: 0.42 + 0.04 * Double(index))
                    }
                }
                .padding(.top, -4)
            }
        }
        .onAppear { shown = true }
    }

    private func chip(_ phrase: String) -> some View {
        Text(phrase)
            .font(WizardFont.chip)
            .padding(.horizontal, 14)
            .padding(.vertical, 4)
            .frame(minHeight: 32)
    }
}

/// Lays its subviews out as `RecipientFlowLayout` does, each line centred.
struct CentredFlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let limit = proposal.width ?? .infinity
        return centredFlowGeometry(sizes: measured(subviews, limit), limit: limit, spacing: spacing).size
    }

    func placeSubviews(
        in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()
    ) {
        let sizes = measured(subviews, bounds.width)
        let flow = centredFlowGeometry(sizes: sizes, limit: bounds.width, spacing: spacing)
        for (index, subview) in subviews.enumerated() {
            let origin = flow.positions[index]
            subview.place(
                at: CGPoint(x: bounds.minX + origin.x, y: bounds.minY + origin.y),
                proposal: ProposedViewSize(sizes[index])
            )
        }
    }

    private func measured(_ subviews: Subviews, _ limit: CGFloat) -> [CGSize] {
        subviews.map { $0.sizeThatFits(ProposedViewSize(width: limit, height: nil)) }
    }
}
