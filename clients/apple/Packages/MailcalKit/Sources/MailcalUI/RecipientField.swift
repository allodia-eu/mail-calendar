// One recipient field (To / Cc / Bcc): the finished addresses as pills, the one being typed as
// text, and the autosuggest list under it.
//
// The field's value stays a single comma-separated **String** owned by the composer, because that is
// what the send path parses, the pills are a rendering of it, not a second source of truth. So
// there is no state to keep in sync and nothing can be on screen that would not be sent.
// `RecipientTokens.swift` owns the string ↔ (pills, token) split in pure functions the test suite
// drives directly.
//
// Two things this buys over the plain text field it replaces:
//
//   * **Each address is visibly one thing.** In a bare field, `a@x.com, b@y.com` is a wall of text
//     whose only boundary is a comma the reader has to find; a duplicated or wrong address is easy
//     to miss, and there is nothing to click to remove one.
//   * **The caret ends up where you would put it.** Accepting a suggestion turns that address into a
//     pill and empties the input, so the caret has nowhere to be but the end, the structural fix for
//     the "next keystroke lands inside the address just inserted" bug, rather than a correction
//     applied after the fact.

import MailcalBindings
import SwiftUI

/// How long the field waits after the last keystroke before asking the core.
///
/// Long enough that typing a word costs one query rather than one per character, short enough that
/// the list still arrives while the user is looking at the field.
private let suggestionDebounce: Duration = .milliseconds(120)

/// A recipient field: pills for the finished addresses, a text input for the one in progress.
///
/// `text` is the whole comma-separated field, so the composer's state shape is unchanged and
/// nothing about submitting a message had to move. `suggestionsFor` is the core lookup; `nil`
/// disables autosuggest entirely. `trailing` is a control drawn beside the input, the To row
/// carries the Cc/Bcc chevron there; Cc and Bcc themselves carry nothing.
struct RecipientField<Trailing: View>: View {
    let label: String
    @Binding var text: String
    var suggestionsFor: ((String) async -> [RecipientMatch])?
    /// Whether the composer opens with the caret in this field.
    var focusesOnAppear = false
    let trailing: Trailing

    /// The token being typed, held apart from `text` so the input shows only the address in
    /// progress while the finished ones render as pills.
    @State private var input = ""
    @State private var matches: [RecipientMatch] = []
    @FocusState private var inputFocused: Bool

    init(
        label: String,
        text: Binding<String>,
        suggestionsFor: ((String) async -> [RecipientMatch])? = nil,
        focusesOnAppear: Bool = false,
        @ViewBuilder trailing: () -> Trailing
    ) {
        self.label = label
        self._text = text
        self.suggestionsFor = suggestionsFor
        self.focusesOnAppear = focusesOnAppear
        self.trailing = trailing()
    }

    private var committed: [String] { committedRecipients(text) }
    private var token: String { currentRecipientToken(text) }

    /// Whether this field's list is on screen.
    ///
    /// Gated on FOCUS as well as on having something to offer. Without that, moving from To to Cc
    /// leaves To's list floating over Cc, harmless while the list sat in the layout, and covering
    /// live content the moment it floats. Tapping a suggestion does not blur the input on either
    /// platform (a plain SwiftUI button takes no responder), so the list is still up when the tap
    /// lands.
    private var showsSuggestions: Bool {
        inputFocused && shouldShowRecipientSuggestions(text, matches.map(\.email))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !committed.isEmpty {
                RecipientFlowLayout {
                    ForEach(Array(committed.enumerated()), id: \.offset) { index, recipient in
                        RecipientPill(recipient: recipient) {
                            text = removeRecipient(text, at: index)
                        }
                    }
                }
            }
            HStack(spacing: 8) {
                TextField(label, text: $input).fieldConfig(.email).focused($inputFocused)
                trailing
            }
            // The suggestions float OVER what follows rather than sitting in the layout, and the
            // field publishes them rather than drawing them: only the composer can put them above
            // the editor (`RecipientSuggestionOverlay`).
            //
            // What floating buys: nothing below this field moves while the user types the first
            // recipient. Inline, the list was a third child of this stack, it appeared and
            // vanished on every keystroke and took its height with it, so Cc, Bcc, Subject, the
            // editor and Send all jumped down and back. On the phone that reaches further than it
            // looks: the composer's measured header drives the editor's top inset, so the message
            // body moved too.
            .anchorPreference(key: RecipientSuggestionOverlayKey.self, value: .bounds) { anchor in
                guard showsSuggestions else { return nil }
                return RecipientSuggestionOverlay(anchor: anchor, matches: matches) { email in
                    text = acceptRecipientSuggestion(text, email)
                }
            }
        }
        .onAppear {
            input = token
            // The caret opens in the field the user has to start in, an empty To on a new message.
            //
            // Deferred by one turn rather than set here: on iOS the composer is a
            // `fullScreenCover`, and focus assigned while the presentation is still animating is
            // dropped on the floor. The keyboard then never rises and the field only *looks* ready.
            if focusesOnAppear {
                Task { @MainActor in inputFocused = true }
            }
        }
        .onChange(of: input) { _, typed in
            text = recipientFieldText(committed, typed)
        }
        // Re-seed the input whenever the token changes underneath us: a suggestion was accepted, a
        // pill was removed, a reply pre-filled the field.
        //
        // The comparison is TRIMMED, and that is the whole point of it. `currentRecipientToken`
        // trims, so the token derived from the field has lost any space the user just typed:
        // compare raw and typing "John " re-seeds the input as "John", eating the space. Every
        // space goes silently: "John Smith" arrives as "JohnSmith", and a name-based autosuggest
        // query can never match anything. Trimming both sides means only a *real* change of token
        // resets the field, and the user's own whitespace is left alone.
        .onChange(of: token) { _, updated in
            if input.trimmingCharacters(in: .whitespaces) != updated { input = updated }
        }
        // Keyed on the TOKEN alone: `.task(id:)` cancels and restarts the query when it changes, so
        // a burst of keystrokes costs one lookup and a result whose token has been superseded can
        // never land. Keying on the closure instead would restart on every render, the composer
        // builds it inline over the model, so it has a fresh identity each pass, and an effect that
        // keeps restarting never finishes a query.
        .task(id: token) { await loadSuggestions(token) }
    }

    /// Debounced, off-the-main-thread lookup. `recipientSuggestions` blocks on the core's runtime
    /// and reaches the store's connection thread three times (people, interaction history,
    /// coverage), so a per-keystroke call in the render path would stall the composer whenever a
    /// sync held that connection, the model hops it to a detached task, and this spaces it out.
    private func loadSuggestions(_ query: String) async {
        guard let suggestionsFor, !query.isEmpty else {
            matches = []
            return
        }
        try? await Task.sleep(for: suggestionDebounce)
        guard !Task.isCancelled else { return }
        matches = await suggestionsFor(query)
    }
}

/// The three-argument form, for a field with no trailing control.
extension RecipientField where Trailing == EmptyView {
    init(
        label: String,
        text: Binding<String>,
        suggestionsFor: ((String) async -> [RecipientMatch])? = nil,
        focusesOnAppear: Bool = false
    ) {
        self.init(
            label: label,
            text: text,
            suggestionsFor: suggestionsFor,
            focusesOnAppear: focusesOnAppear
        ) { EmptyView() }
    }
}

/// One finished recipient, with its own remove control.
private struct RecipientPill: View {
    let recipient: String
    let remove: () -> Void

    var body: some View {
        HStack(spacing: 4) {
            Text(recipient)
                .font(.callout)
                .lineLimit(1)
                .truncationMode(.middle)
            Button(action: remove) {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            // Names the recipient, so the control is distinguishable when a screen reader reaches
            // the third otherwise-identical "Remove recipient" button.
            .accessibilityLabel("\(recipient), \(L10n.compose_remove_recipient())")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(.quaternary.opacity(0.6), in: Capsule())
    }
}

/// Lays its subviews left to right, wrapping onto a new line when the next one does not fit.
///
/// SwiftUI has no flow container, and the alternatives are wrong for pills: an `HStack` truncates a
/// long list into one unreadable line, and a `LazyVGrid` gives every recipient a column of the same
/// width whether it is `jo@x.eu` or a 40-character address.
///
/// A thin adapter over `recipientFlowGeometry`, which holds the arithmetic and the test suite.
struct RecipientFlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let limit = proposal.width ?? .infinity
        return recipientFlowGeometry(
            sizes: measured(subviews, limit), limit: limit, spacing: spacing
        ).size
    }

    func placeSubviews(
        in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()
    ) {
        let sizes = measured(subviews, bounds.width)
        let flow = recipientFlowGeometry(sizes: sizes, limit: bounds.width, spacing: spacing)
        for (index, subview) in subviews.enumerated() {
            let origin = flow.positions[index]
            subview.place(
                at: CGPoint(x: bounds.minX + origin.x, y: bounds.minY + origin.y),
                proposal: ProposedViewSize(sizes[index])
            )
        }
    }

    /// Every pill measured against the width the row actually has, never `.unspecified`.
    ///
    /// Unconstrained, a pill answers with the address's full ideal width, so its `truncationMode`
    /// never engages and the layout goes on to claim a container wider than the screen.
    private func measured(_ subviews: Subviews, _ limit: CGFloat) -> [CGSize] {
        subviews.map { $0.sizeThatFits(ProposedViewSize(width: limit, height: nil)) }
    }
}
