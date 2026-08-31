// The composer's floating recipient-suggestion layer: the list a recipient field offers, drawn by
// the composer instead of by the field.
//
// The list must take no layout space, a list that appears and disappears on every keystroke and
// takes a field's height with it moves Cc, Bcc, Subject, the editor and Send while the user is
// still typing the first recipient (docs/contacts.md §4). An overlay inside the field achieves
// that much, and is still not enough: **the editor is a `WKWebView`, and SwiftUI composites a
// hosted UIKit view above its own drawing**, `zIndex` does not move it, and neither does an
// overlay on the scroll view the web view sits in. Both were tried; both left the list sliced off
// at the editor's top edge, drawn over the fields and under the message.
//
// What clears it is distance: the layer goes on the **root of the composer**, outside the
// navigation stack. So the field publishes where its input is and what to offer, and
// `RichComposeView` draws it up there.
//
// At most one is ever published: a field offers suggestions only while it has focus.

import MailcalBindings
import SwiftUI

/// One field's floating list: the input's frame to hang it under, the addresses to offer, and how
/// to apply the one that is picked.
struct RecipientSuggestionOverlay {
    let anchor: Anchor<CGRect>
    let matches: [RecipientMatch]
    let accept: (String) -> Void
}

/// Carries the open list up to the composer. `reduce` keeps the first: only the focused field
/// publishes, so there is never a second one to choose between.
enum RecipientSuggestionOverlayKey: PreferenceKey {
    static let defaultValue: RecipientSuggestionOverlay? = nil

    static func reduce(
        value: inout RecipientSuggestionOverlay?,
        nextValue: () -> RecipientSuggestionOverlay?
    ) {
        value = value ?? nextValue()
    }
}

extension View {
    /// Draws whichever recipient field is offering suggestions, over this view.
    ///
    /// Apply it to the composer's outermost view. Nearer the field it loses to the editor's web
    /// view, that is the whole point; see this file's header.
    func recipientSuggestionLayer() -> some View {
        overlayPreferenceValue(RecipientSuggestionOverlayKey.self) { open in
            GeometryReader { proxy in
                if let open {
                    let input = proxy[open.anchor]
                    RecipientSuggestionList(matches: open.matches, accept: open.accept)
                        // The input's own width, so a long address cannot widen the list past the
                        // field it belongs to.
                        .frame(width: input.width, alignment: .leading)
                        .offset(x: input.minX, y: input.maxY + 4)
                }
            }
            // The layer spans the whole composer, so with no list open it must be transparent to
            // touch: left hit-testable it swallows every tap meant for the fields under it, and
            // Cc, Bcc and Subject stop responding while looking perfectly normal.
            .allowsHitTesting(open != nil)
        }
    }
}

/// The list itself.
///
/// Stale matches stay on screen for the debounce window rather than blanking, which is what makes
/// the list feel steady instead of flickering on every character. The count is capped in the core,
/// not here.
///
/// The surface is OPAQUE and shadowed because it covers live content: a translucent fill renders
/// two texts on top of each other once the list floats.
private struct RecipientSuggestionList: View {
    let matches: [RecipientMatch]
    let accept: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(matches, id: \.email) { match in
                Button { accept(match.email) } label: {
                    VStack(alignment: .leading, spacing: 1) {
                        // A suggestion that came only from sent mail carries no name. It is as
                        // valid as one from a saved card, and usually the more useful, so it
                        // shows its address alone rather than being hidden (docs/contacts.md §4).
                        if !match.displayName.isEmpty {
                            Text(match.displayName).font(.callout)
                        }
                        Text(match.email)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 8)
                .fill(.background)
                .shadow(color: .black.opacity(0.18), radius: 8, y: 3)
        }
        .overlay {
            RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary)
        }
    }
}
