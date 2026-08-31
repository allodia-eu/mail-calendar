// The quote-style setting: how a reply or forward quotes the original. Two named styles, each shown
// as a worked example rather than described in words, the names alone ("Indented", "Line + header")
// don't tell you what you'd get, and the preview does. Below them, an opt-in toggle that puts the
// same choice in every composer so a single message can deviate from the default without changing it.
//
// One view, used by both settings surfaces (the macOS Settings window and the iOS settings sheet),
// so the two can't drift. The example content comes from `ComposerQuote.example`, which builds it
// from the same catalog keys a real quote uses; the rendering mirrors the shared editor's CSS
// (clients/composer/dist/editor.html) and the Rust renderer (mailcal-composer).

import MailcalBindings
import SwiftUI

/// The app-level default quote style, each option previewed, plus the per-message opt-in.
struct QuoteStyleSettings: View {
    var model: MailboxModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            QuoteStyleOption(
                label: L10n.quote_style_indented(),
                description: L10n.quote_style_indented_description(),
                style: .indented,
                selected: model.quoteSettings.style == .indented
            ) { model.setQuoteStyle(.indented) }
            QuoteStyleOption(
                label: L10n.quote_style_line_header(),
                description: L10n.quote_style_line_header_description(),
                style: .lineAndHeader,
                selected: model.quoteSettings.style == .lineAndHeader
            ) { model.setQuoteStyle(.lineAndHeader) }
            Divider()
            Toggle(isOn: perMessageBinding) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(L10n.settings_quote_per_message_heading())
                    Text(L10n.settings_quote_per_message_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var perMessageBinding: Binding<Bool> {
        Binding(
            get: { model.quoteSettings.perMessage },
            set: { model.setQuoteStylePerMessage($0) }
        )
    }
}

/// One style: a radio row with its name and a plain-language description, and the live example.
private struct QuoteStyleOption: View {
    let label: String
    let description: String
    let style: QuoteStyleKind
    let selected: Bool
    let select: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button(action: select) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: selected ? "largecircle.fill.circle" : "circle")
                        .foregroundStyle(selected ? Color.accentColor : Color.secondary)
                    Text(label)
                        .foregroundStyle(.primary)
                }
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(selected ? [.isSelected, .isButton] : .isButton)

            Text(description)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            QuoteStyleExample(style: style)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
}

/// The worked example. Deliberately not an editor, just enough of the shape (the indent and left
/// rule, or the divider and labelled header block) to recognise at a glance which one you want.
private struct QuoteStyleExample: View {
    let style: QuoteStyleKind

    var body: some View {
        let example = ComposerQuote.example()
        VStack(alignment: .leading, spacing: 6) {
            if style == .indented {
                Text(example.line)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                HStack(alignment: .top, spacing: 8) {
                    // The left rule + inset the indented style renders the original in.
                    Rectangle()
                        .fill(Color.secondary.opacity(0.4))
                        .frame(width: 2)
                    Text(example.body).font(.caption2)
                }
                .fixedSize(horizontal: false, vertical: true)
            } else {
                // The divider the original is set off by, then the header block at full width.
                Rectangle()
                    .fill(Color.secondary.opacity(0.4))
                    .frame(height: 1)
                ForEach(example.headers, id: \.label) { header in
                    (Text("\(header.label): ").bold() + Text(header.value))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Text(example.body).font(.caption2)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 6))
    }
}
