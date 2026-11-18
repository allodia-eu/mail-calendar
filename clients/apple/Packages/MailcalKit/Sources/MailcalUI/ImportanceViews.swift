// A message's importance on screen (docs/importance.md): the mark a list row draws before the
// flag, and the banner above an open message. Which importance a row or a message has, and whether
// a message carries a banner, is the core's answer; this only draws it.

import MailcalBindings
import SwiftUI

/// The symbol and colour for an importance, or `nil` for normal, which draws nothing. Red for
/// high, as the mark that asks for attention, and blue for low, as Outlook draws them.
private func importanceSymbol(_ importance: MessageImportance) -> (symbol: String, colour: Color)? {
    switch importance {
    case .high: ("exclamationmark", .red)
    case .low: ("arrow.down", .blue)
    case .normal: nil
    }
}

/// The list row's mark; nothing for a normal message.
struct ImportanceMark: View {
    let importance: MessageImportance

    var body: some View {
        if let glyph = importanceSymbol(importance) {
            Image(systemName: glyph.symbol)
                .fontWeight(.bold)
                .foregroundStyle(glyph.colour)
                .accessibilityLabel(
                    importance == .high ? L10n.a11y_importance_high() : L10n.a11y_importance_low()
                )
        }
    }
}

/// The banner above an open message the core asked for; nothing for `nil`.
struct ImportanceBanner: View {
    let importance: MessageImportance?

    var body: some View {
        if let importance, let glyph = importanceSymbol(importance) {
            HStack(spacing: 8) {
                Image(systemName: glyph.symbol)
                    .fontWeight(.bold)
                    .foregroundStyle(glyph.colour)
                    .accessibilityHidden(true)
                Text(
                    importance == .high
                        ? L10n.reading_importance_high() : L10n.reading_importance_low()
                )
                .font(.caption)
                Spacer()
            }
            .padding(.horizontal, readingInset)
            .padding(.vertical, 8)
            .background(.quaternary)
        }
    }
}

/// What the composer's importance menu calls each entry.
func importanceLabel(_ importance: MessageImportance) -> String {
    switch importance {
    case .high: L10n.compose_importance_high()
    case .normal: L10n.compose_importance_normal()
    case .low: L10n.compose_importance_low()
    }
}
