// Draft a reply's two surfaces of its own (docs/ai.md rules 18, 19 and 23): the instruction's
// input the control opens, and the card a draft brings among the composer's notes. Both draw the
// record and word it; neither is part of the mail.

import MailcalBindings
import SwiftUI

/// The instruction's input: one optional line with the core's suggestions under it and Draft, or
/// in its place why Draft cannot be pressed, with its one remedy.
struct ReplyDraftInput: View {
    let record: ComposerReplyDraft
    @Binding var line: String
    let draft: () -> Void
    let remedy: (AiRemedy) -> Void

    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.composer_draft_reply())
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            if let held = record.heldBy {
                Label(AiWords.failure(held.failure), systemImage: "info.circle")
                    .fixedSize(horizontal: false, vertical: true)
                if let fix = held.remedy {
                    ControlButton(title: AiWords.remedy(fix.remedy), control: fix.control) {
                        remedy(fix.remedy)
                    }
                    .buttonStyle(.borderedProminent)
                }
            } else {
                TextField(L10n.composer_draft_intent_hint(), text: $line, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...3)
                    .focused($focused)
                    .onSubmit(draft)
                    .submitLabel(.go)
                    .accessibilityIdentifier("composer.draftInstruction")
                suggestions
                HStack {
                    Spacer()
                    ControlButton(
                        title: L10n.composer_draft_create(), control: record.draft,
                        shortcut: .defaultAction, action: draft
                    )
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("composer.draftCreate")
                }
            }
        }
        .padding(18)
        #if os(macOS)
        .frame(width: 360)
        #else
        .frame(minWidth: 320, maxWidth: .infinity, alignment: .topLeading)
        #endif
        .onAppear { focused = record.heldBy == nil }
    }

    /// The core's suggestions, each filling the line with its words: a flow of capsules that
    /// wraps at a large text size rather than cutting a word.
    private var suggestions: some View {
        RecipientFlowLayout(spacing: 8) {
            ForEach(Array(record.suggestions.enumerated()), id: \.offset) { _, suggestion in
                let words = ReplyDraftWords.suggestion(suggestion)
                Button(words) { line = words }
                    .buttonStyle(.bordered)
                    .buttonBorderShape(.capsule)
                    .controlSize(.small)
            }
        }
    }
}

/// The card a draft brings: what the message asks, then what is left to do before sending. A
/// gap's tick follows the reply; the others are the person's.
struct ReplyDraftCard: View {
    let summary: String
    let checklist: [ReplyDraftItem]
    let tick: (_ index: UInt32, _ done: Bool) -> Void

    @ScaledMetric(relativeTo: .callout) private var iconWidth = 18.0

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !summary.isEmpty {
                heading(L10n.composer_draft_summary_heading())
                Text(summary)
                    .font(.callout)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if !checklist.isEmpty {
                heading(L10n.composer_checklist_heading())
                    .padding(.top, summary.isEmpty ? 0 : 4)
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(checklist, id: \.index) { item in
                        ControlCheck(
                            title: ReplyDraftWords.item(item), done: item.done, control: item.tick,
                            symbol: symbol(item.kind), iconWidth: iconWidth
                        ) { tick(item.index, $0) }
                    }
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("composer.draftCard")
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(Color.secondary)
            .accessibilityAddTraits(.isHeader)
    }

    private func symbol(_ kind: ReplyDraftTaskKind) -> String {
        switch kind {
        case .fillIn: "character.cursor.ibeam"
        case .attach: "paperclip"
        case .do: "checklist"
        }
    }
}
