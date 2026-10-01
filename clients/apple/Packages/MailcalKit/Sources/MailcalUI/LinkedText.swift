// Sender text drawn natively with its web and mail addresses as links: a plain-text body, an
// event's notes, an invitation's description.
//
// The core finds the addresses (`linkedText(text:)`), so this client and every other link the same
// ones. Each run is appended as text through `AttributedString(String)`, which parses nothing, and
// `.link` is set only from the core's target, so `**bold**` or `<b>` stays literal and Gate 8
// (docs/rendering-security.md) holds. Opening goes through `shouldOpenExternalLink`, the gate the
// reading web view asks, and the OS opens what it allows.
//
// macOS draws no pointing hand over a link in `Text`, so `gatedLinkOpening()` adds one: each link
// run carries `LinkRun`, which the laid-out text reports back with its bounds.

import Foundation
import MailcalBindings
import SwiftUI

extension LinkedText {
    /// The runs as one value for `Text`, a run with a parseable target carrying it as `.link`.
    static func attributed(_ runs: [LinkedText]) -> AttributedString {
        var result = AttributedString()
        for run in runs {
            var piece = AttributedString(run.text)
            if let link = run.link, let url = URL(string: link) {
                piece.link = url
            }
            result.append(piece)
        }
        return result
    }

    /// `text` with the addresses the core finds in it as links, each link run marked as `LinkRun`.
    ///
    /// The pieces are joined by interpolating `Text` values, whose contents are never parsed; only
    /// the `"\(…)\(…)"` format is a localised key.
    static func text(linkingIn text: String) -> Text {
        let value = attributed(linkedText(text: text))
        return value.runs.reduce(Text(verbatim: "")) { joined, run in
            let piece = Text(AttributedString(value[run.range]))
            let marked = run.link.map { piece.customAttribute(LinkRun(url: $0)) } ?? piece
            return Text("\(joined)\(marked)")
        }
    }
}

/// Marks a link's run in laid-out text with its target, so macOS can lay a pointer over it.
struct LinkRun: TextAttribute {
    let url: URL
}

extension View {
    /// Opens a tapped link only when the shared launch policy allows it; anything else is dropped.
    /// On macOS it also shows the pointing hand over a link.
    func gatedLinkOpening() -> some View {
        #if os(macOS)
        let linked = modifier(LinkPointer())
        #else
        let linked = self
        #endif
        return linked.environment(
            \.openURL,
            OpenURLAction { url in
                shouldOpenExternalLink(url: url.absoluteString) ? .systemAction : .discarded
            }
        )
    }
}

#if os(macOS)
/// A pointing hand over each `LinkRun` of the text inside this view.
///
/// Selectable `Text` sets the I-beam from inside itself, which wins over a `pointerStyle` set from
/// outside, so the hand comes from a clear area laid over each link. It takes the click as well, and
/// opens the link through the same `openURL` the text would have used.
private struct LinkPointer: ViewModifier {
    @Environment(\.openURL) private var openURL

    func body(content: Content) -> some View {
        content.overlayPreferenceValue(Text.LayoutKey.self) { layouts in
            GeometryReader { proxy in
                ForEach(Array(Self.links(in: layouts, proxy: proxy).enumerated()), id: \.offset) {
                    let (url, rect) = $0.element
                    Color.clear
                        .contentShape(Rectangle())
                        .frame(width: rect.width, height: rect.height)
                        .position(x: rect.midX, y: rect.midY)
                        .pointerStyle(.link)
                        .onTapGesture { openURL(url) }
                }
            }
            .accessibilityHidden(true)
        }
    }

    private static func links(
        in layouts: [Text.LayoutKey.AnchoredLayout],
        proxy: GeometryProxy
    ) -> [(URL, CGRect)] {
        layouts.flatMap { anchored in
            let origin = proxy[anchored.origin]
            return anchored.layout.flatMap { line in
                line.compactMap { run in
                    run[LinkRun.self].map {
                        ($0.url, run.typographicBounds.rect.offsetBy(dx: origin.x, dy: origin.y))
                    }
                }
            }
        }
    }
}
#endif
