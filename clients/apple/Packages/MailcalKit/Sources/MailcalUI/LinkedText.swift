// Sender text drawn natively with its web and mail addresses as links: a plain-text body, an
// event's notes, an invitation's description.
//
// The core finds the addresses (`linkedText(text:)`), so this client and every other link the same
// ones. Each run is appended as text through `AttributedString(String)`, which parses nothing, and
// `.link` is set only from the core's target, so `**bold**` or `<b>` stays literal and Gate 8
// (docs/rendering-security.md) holds. Opening goes through `shouldOpenExternalLink`, the gate the
// reading web view asks, and the OS opens what it allows.

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

    /// `text` with the addresses the core finds in it as links.
    static func attributed(linkingIn text: String) -> AttributedString {
        attributed(linkedText(text: text))
    }
}

extension View {
    /// Opens a tapped link only when the shared launch policy allows it; anything else is dropped.
    func gatedLinkOpening() -> some View {
        environment(
            \.openURL,
            OpenURLAction { url in
                shouldOpenExternalLink(url: url.absoluteString) ? .systemAction : .discarded
            }
        )
    }
}
