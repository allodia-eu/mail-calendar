// The thumbs, and the form behind thumbs down (docs/ai.md, "Feedback"), shared by a drafted reply's
// card and the training window. The form offers to include the email and the draft only as feedback,
// never ticked until the person ticks it, and says that the email holds the sender's words too.

import MailcalBindings
import SwiftUI

struct DraftRatingControls: View {
    @Binding var feedback: DraftFeedback
    /// Feedback on a composer's draft: the form offers the email and the draft, and one rating is
    /// final. The training window rates again as often as it likes, with nothing to include.
    let isFeedback: Bool
    /// Keeps a rating, with the email and the draft when asked; answers whether it was kept.
    let keep: (DraftRating, Bool) -> Bool

    var body: some View {
        Group {
            switch feedback.stage {
            case .kept where isFeedback:
                note(L10n.ai_feedback_thanks())
            case .failed:
                note(L10n.ai_feedback_failed())
            default:
                thumbs
            }
        }
        .sheet(isPresented: asking) {
            DraftRatingForm(feedback: $feedback, isFeedback: isFeedback, send: keepDown)
        }
    }

    private var thumbs: some View {
        HStack(spacing: 12) {
            Text(L10n.ai_feedback_prompt())
                .font(.caption)
                .foregroundStyle(Color.secondary)
            Spacer(minLength: 0)
            Button(action: keepUp) {
                Image(systemName: feedback.verdict == .up ? "hand.thumbsup.fill" : "hand.thumbsup")
            }
            .accessibilityLabel(L10n.ai_feedback_good())
            Button { feedback.down() } label: {
                Image(systemName: feedback.verdict == .down ? "hand.thumbsdown.fill" : "hand.thumbsdown")
            }
            .accessibilityLabel(L10n.ai_feedback_poor())
        }
        .buttonStyle(.borderless)
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(Color.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var asking: Binding<Bool> {
        Binding(
            get: { feedback.stage == .asking },
            set: { open in if !open, feedback.stage == .asking { feedback.cancel() } }
        )
    }

    private func keepUp() {
        let rating = feedback.up()
        feedback.settle(rating, kept: keep(rating, false))
    }

    private func keepDown() {
        let rating = feedback.downRating()
        feedback.settle(rating, kept: keep(rating, isFeedback && feedback.includeContent))
    }
}

/// What was wrong, anything to add, and, as feedback, whether the email and the draft go too.
struct DraftRatingForm: View {
    @Binding var feedback: DraftFeedback
    let isFeedback: Bool
    let send: () -> Void

    var body: some View {
        #if os(iOS)
        // The question heads the content rather than the bar, where the two buttons beside it
        // leave it a few letters on a phone.
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text(L10n.ai_feedback_title()).font(.headline)
                    content
                }
                .padding(20)
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel()) { feedback.cancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.ai_feedback_send(), action: send)
                }
            }
        }
        .presentationDetents([.medium, .large])
        #else
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.ai_feedback_title()).font(.headline)
            content
            HStack {
                Spacer()
                Button(L10n.action_cancel(), role: .cancel) { feedback.cancel() }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.ai_feedback_send(), action: send)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 380)
        #endif
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(DraftFeedback.reasons, id: \.self) { reason in
                    Toggle(DraftFeedback.title(reason), isOn: ticked(reason)).feedbackCheckbox()
                }
            }
            TextField(L10n.ai_feedback_comment_hint(), text: $feedback.comment, axis: .vertical)
                .lineLimit(3...6)
                .textFieldStyle(.roundedBorder)
            if isFeedback {
                VStack(alignment: .leading, spacing: 4) {
                    Toggle(L10n.ai_feedback_include(), isOn: $feedback.includeContent)
                        .feedbackCheckbox()
                    caption(L10n.ai_feedback_include_note())
                }
                caption(L10n.ai_feedback_explainer())
            }
        }
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(Color.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func ticked(_ reason: DraftRatingReason) -> Binding<Bool> {
        Binding(
            get: { feedback.reasons.contains(reason) },
            set: { on in
                if on { feedback.reasons.insert(reason) } else { feedback.reasons.remove(reason) }
            }
        )
    }
}

extension View {
    /// A checkbox on both platforms: the Mac's own, and on iOS the square the checklist draws.
    @ViewBuilder func feedbackCheckbox() -> some View {
        #if os(macOS)
        toggleStyle(.checkbox)
        #else
        toggleStyle(SquareCheckbox())
        #endif
    }
}

#if os(iOS)
private struct SquareCheckbox: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button {
            configuration.isOn.toggle()
        } label: {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: configuration.isOn ? "checkmark.square.fill" : "square")
                    .foregroundStyle(configuration.isOn ? Color.accentColor : Color.secondary)
                    .accessibilityHidden(true)
                configuration.label
                    .foregroundStyle(Color.primary)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(configuration.isOn ? [.isToggle, .isSelected] : .isToggle)
    }
}
#endif
