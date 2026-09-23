// The composer's Draft a reply control (docs/ai.md, "Drafting a reply"): a button in the action
// bar beside Signature, a small popover with the one-line intent, and the draft going into the open
// composer above the signature and the quote. Nothing on this path sends. Split out of
// RichComposerView.swift to keep it under 500 lines.

import Foundation
import MailcalBindings
import SwiftUI

/// What a reply composer needs to offer a draft: the message being answered, and the two calls the
/// core answers. Passed as a value, like `ComposerSignatures`; `nil` means no control at all, which
/// is every composer that is not a reply or a reply all, and every one while AI has nowhere to go.
struct ComposerDraftReply {
    /// The account that received the message being answered, and its key.
    let account: String
    let key: String
    /// Where requests go, for the one failure whose wording depends on it.
    let route: AiRoute
    /// The style an account drafts in, or `nil` when it has none.
    let styleFor: @MainActor (String) -> String?
    /// Drafts the reply off the main actor: the sending account when it is not `account`, and the
    /// intent when there is one.
    let draft: @MainActor (_ from: String?, _ intent: String?) async
        -> Result<DraftReply, WritingStyleFailure>
}

/// The control's state, held once by the composer so the status beside the body outlives the
/// popover that started it.
@MainActor
@Observable
final class ComposerDraftStatus {
    var popoverOpen = false
    var intent = ""
    /// The draft is being written; the composer stays usable meanwhile.
    var drafting = false
    /// The last draft left gaps in brackets. Shown until the next draft or send.
    var checkBrackets = false
    var failure: String?
    /// A draft waiting on "replace what you have written?", with the intent it was asked with.
    var confirmingReplace = false
    var pendingIntent: String?
}

extension RichComposeView {
    /// The button in the action bar. Disabled, with the reason as its hint, while the sending
    /// account has no style. A `Button` with a popover rather than a `Menu`, for the reason the
    /// reading pane's overflow gives (docs/client-traps.md).
    func draftReplyControl(_ draftReply: ComposerDraftReply) -> some View {
        let hasStyle = resolvedFrom.flatMap(draftReply.styleFor) != nil
        return Button {
            draftStatus.popoverOpen = true
        } label: {
            Label(L10n.composer_draft_reply(), systemImage: "text.quote")
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .fixedSize()
        .disabled(!hasStyle || draftStatus.drafting)
        .help(hasStyle ? L10n.composer_draft_reply() : L10n.ai_error_no_style())
        .accessibilityHint(hasStyle ? "" : L10n.ai_error_no_style())
        .popover(isPresented: $draftStatus.popoverOpen, arrowEdge: .bottom) {
            DraftReplyPopover(status: draftStatus) { requestDraft(draftReply) }
                // A half-height sheet on a phone, where a popover has no room beside the keyboard.
                .presentationDetents([.medium])
        }
        .alert(L10n.composer_draft_replace_title(), isPresented: $draftStatus.confirmingReplace) {
            Button(L10n.composer_draft_replace(), role: .destructive) {
                runDraft(draftReply, intent: draftStatus.pendingIntent)
            }
            Button(L10n.action_cancel(), role: .cancel) { draftStatus.pendingIntent = nil }
        } message: {
            Text(L10n.composer_draft_replace_message())
        }
    }

    /// "Drafting…", a failure, or the reminder to check the gaps, near the body.
    @ViewBuilder var draftStatusLine: some View {
        if draftStatus.drafting {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text(L10n.composer_drafting()).font(.caption).foregroundStyle(.secondary)
            }
        } else if let failure = draftStatus.failure {
            Text(failure)
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        } else if draftStatus.checkBrackets {
            Label(L10n.composer_draft_check_brackets(), systemImage: "exclamationmark.circle")
                .font(.caption)
                .foregroundStyle(.orange)
        }
    }

    /// Asks before a draft replaces what the person has written above the signature and the quote.
    private func requestDraft(_ draftReply: ComposerDraftReply) {
        let intent = draftReplyIntent(draftStatus.intent)
        draftStatus.popoverOpen = false
        Task {
            if await editor.leadHasText() {
                draftStatus.pendingIntent = intent
                draftStatus.confirmingReplace = true
            } else {
                runDraft(draftReply, intent: intent)
            }
        }
    }

    private func runDraft(_ draftReply: ComposerDraftReply, intent: String?) {
        draftStatus.pendingIntent = nil
        draftStatus.failure = nil
        draftStatus.checkBrackets = false
        draftStatus.drafting = true
        let from = draftReplyFrom(answered: draftReply.account, sending: resolvedFrom)
        Task {
            let result = await draftReply.draft(from, intent)
            draftStatus.drafting = false
            switch result {
            case let .success(draft):
                editor.setDraftText(draft.text, draftId: draft.draftId)
                draftStatus.checkBrackets = !draft.gaps.isEmpty
                draftStatus.intent = ""
            case let .failure(failure):
                draftStatus.failure = writingStyleFailureText(failure, route: draftReply.route)
            }
        }
    }
}

/// The popover: what the reply should say, four answers that fill it in, and Draft.
private struct DraftReplyPopover: View {
    @Bindable var status: ComposerDraftStatus
    let create: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            TextField(L10n.composer_draft_intent_hint(), text: $status.intent)
                .textFieldStyle(.roundedBorder)
                .onSubmit(create)
            RecipientFlowLayout(spacing: 6) {
                chip(L10n.composer_draft_chip_yes())
                chip(L10n.composer_draft_chip_no())
                chip(L10n.composer_draft_chip_more_info())
                chip(L10n.composer_draft_chip_later())
            }
            HStack {
                Spacer()
                Button(L10n.composer_draft_create(), action: create)
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .frame(width: 340)
    }

    private func chip(_ text: String) -> some View {
        Button(text) { status.intent = text }
            .buttonStyle(.bordered)
            .controlSize(.small)
    }
}
