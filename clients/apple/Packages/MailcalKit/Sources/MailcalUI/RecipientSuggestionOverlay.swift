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

import Combine
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
    // `nonisolated(unsafe)` because the value carries the field's `accept` closure and so is not
    // `Sendable`, while `PreferenceKey` requires this to be reachable from anywhere. Nothing is
    // shared: it is `nil`, and every real value is made and read on the main actor by SwiftUI.
    nonisolated(unsafe) static let defaultValue: RecipientSuggestionOverlay? = nil

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
        modifier(RecipientSuggestionLayer())
    }
}

private struct RecipientSuggestionLayer: ViewModifier {
    #if os(iOS)
        /// The on-screen keyboard's frame on screen, or `nil` while it is down. Read from the
        /// keyboard itself rather than from the safe area: hosts disagree about whether the
        /// keyboard has already shrunk this layer, and one that has still reports it as an inset.
        @State private var keyboard: CGRect?
    #endif

    func body(content: Content) -> some View {
        content
            .overlayPreferenceValue(RecipientSuggestionOverlayKey.self) { open in
                GeometryReader { proxy in
                    if let open { list(open, in: proxy) }
                }
                // The layer spans the whole composer, so with no list open it must be transparent
                // to touch: left hit-testable it swallows every tap meant for the fields under it,
                // and Cc, Bcc and Subject stop responding while looking perfectly normal.
                .allowsHitTesting(open != nil)
            }
        #if os(iOS)
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillChangeFrameNotification)) {
                keyboard = $0.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
                keyboard = nil
            }
        #endif
    }

    @ViewBuilder
    private func list(_ open: RecipientSuggestionOverlay, in proxy: GeometryProxy) -> some View {
        let input = proxy[open.anchor]
        #if os(iOS)
            // Clear of the on-screen keyboard and across the window (docs/contacts.md §4): the
            // layer is a sibling of the form, so the keyboard does not resize the list, and nothing
            // but this keeps it off the keyboard.
            let keyboardTop = keyboard.map { $0.minY - proxy.frame(in: .global).minY } ?? .infinity
            let placement = recipientSuggestionPlacement(
                field: input,
                visibleTop: proxy.safeAreaInsets.top,
                visibleBottom: min(proxy.size.height, keyboardTop)
            )
            RecipientSuggestionList(matches: open.matches, accept: open.accept)
                // A box as tall as the room, with the list at the edge nearest the field: above,
                // it then ends just over the input whatever its own height. The box's height is
                // also what the list measures itself against, and scrolls past.
                .frame(
                    width: proxy.size.width - 2 * suggestionSideMargin,
                    height: placement.maxHeight,
                    alignment: placement.above ? .bottom : .top
                )
                .offset(
                    x: suggestionSideMargin,
                    y: placement.above
                        ? input.minY - suggestionGap - placement.maxHeight
                        : input.maxY + suggestionGap
                )
        #else
            // A desktop's window keeps its content on screen, so the list stays the input's own
            // width, under it, and a long address cannot widen it past the field it belongs to.
            RecipientSuggestionList(matches: open.matches, accept: open.accept)
                .frame(width: input.width, alignment: .leading)
                .offset(x: input.minX, y: input.maxY + suggestionGap)
        #endif
    }
}

/// The space between the input and its list.
private let suggestionGap: CGFloat = 4
/// How far the list keeps from the window's sides on a phone or tablet.
private let suggestionSideMargin: CGFloat = 8

/// Where a field's list goes on a phone or tablet: which side of the input, and how tall it may be.
struct RecipientSuggestionPlacement: Equatable {
    let above: Bool
    let maxHeight: CGFloat
}

/// The side and height of a field's list, from the room the field leaves in the **visible** part
/// of the window: the part the keyboard does not cover (docs/contacts.md §4). Android's
/// `suggestionPlacement` is the same rule.
///
/// Under the field when there is room for about three suggestions there (`minRoom`), or at least
/// as much as above it; otherwise above. Either way no taller than that room, so the list never
/// reaches under the keyboard and scrolls instead. The event editor's invitee field is the case
/// that needs the other side: the form scrolls it to just above the keyboard, which leaves no room
/// under it.
func recipientSuggestionPlacement(
    field: CGRect,
    visibleTop: CGFloat,
    visibleBottom: CGFloat,
    gap: CGFloat = suggestionGap,
    minRoom: CGFloat = 160
) -> RecipientSuggestionPlacement {
    let below = visibleBottom - field.maxY - gap
    let above = field.minY - visibleTop - gap
    if below >= minRoom || below >= above {
        return RecipientSuggestionPlacement(above: false, maxHeight: max(below, 0))
    }
    return RecipientSuggestionPlacement(above: true, maxHeight: max(above, 0))
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
        // As tall as its rows while they fit the height it is offered, and a scroll view of exactly
        // that height when they do not, so a short list is not padded out to the full room.
        ViewThatFits(in: .vertical) {
            surface(rows)
            surface(ScrollView { rows })
        }
    }

    private func surface(_ content: some View) -> some View {
        content
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .background {
                RoundedRectangle(cornerRadius: 8)
                    .fill(.background)
                    .shadow(color: .black.opacity(0.18), radius: 8, y: 3)
            }
            .overlay {
                RoundedRectangle(cornerRadius: 8).strokeBorder(.quaternary)
            }
    }

    private var rows: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(matches, id: \.email) { match in
                VStack(alignment: .leading, spacing: 1) {
                    // A suggestion that came only from sent mail carries no name. It is as valid as
                    // one from a saved card, and usually the more useful, so it shows its address
                    // alone rather than being hidden (docs/contacts.md §4).
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
                // A tap, not a `Button`: a button fires when the finger lifts anywhere inside it,
                // and a slow drag through the list is not always taken by the scroll view first,
                // so scrolling picked whichever address the finger came to rest on. A tap fails
                // the moment the finger moves. Still a button to assistive technology.
                .onTapGesture { accept(match.email) }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isButton)
                .accessibilityAction { accept(match.email) }
            }
        }
    }
}
