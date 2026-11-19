// Draft a reply in the composer (docs/ai.md rules 17 to 24), drawn from the core's
// `composer_reply_draft`: the control in the action row beside Signature, the instruction's input
// it opens, what it says among the composer's notes, and the questions it asks before replacing
// the person's text and before Send. The record decides each; this draws and words it.

import MailcalBindings
import SwiftUI

extension RichComposeView {
    /// Draft a reply, beside Signature, from the record's `control`: running while a draft is
    /// made, held only when the message answered is gone. It opens the instruction's input, a
    /// popover on a Mac and an iPad, and a sheet at half height on an iPhone.
    @ViewBuilder var draftReplyControl: some View {
        if let record = replyDraft.record {
            ControlButton(
                title: L10n.composer_draft_reply(), systemImage: "text.append",
                control: record.control, action: openDraftInput
            )
            #if os(macOS)
            .buttonStyle(.bordered)
            .controlSize(.small)
            #endif
            .help(L10n.a11y_composer_draft_reply_hint())
            .accessibilityHint(L10n.a11y_composer_draft_reply_hint())
            .accessibilityIdentifier("composer.draftReply")
            .popover(isPresented: $replyDraft.inputShown, arrowEdge: Self.draftInputEdge) {
                ReplyDraftInput(
                    record: record, line: $replyDraft.line, draft: pressDraft,
                    remedy: takeRemedy
                )
                .presentationDetents([.medium])
                .presentationCompactAdaptation(.sheet)
            }
        }
    }

    /// Where the input opens from the control. On a Mac the control ends the action row at the
    /// composer's trailing side, often the screen's, and a popover below it runs off the screen
    /// there rather than sliding back, so it opens to the control's leading side.
    private static var draftInputEdge: Edge {
        #if os(macOS)
        .leading
        #else
        .bottom
        #endif
    }

    /// What Draft a reply says among the composer's notes: the draft being made with its Stop, the
    /// message answered gone, a failure with its remedy, or the card.
    @ViewBuilder var replyDraftNotes: some View {
        if let record = replyDraft.record {
            switch record.state {
            case .drafting:
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.composer_drafting()).font(.callout).foregroundStyle(.secondary)
                    Spacer(minLength: 8)
                    ControlButton(title: L10n.composer_draft_stop(), control: record.stop) {
                        replyDrafts?.change(.stop(composition: composition))
                    }
                    .accessibilityIdentifier("composer.draftStop")
                }
                .onAppear {
                    AccessibilityNotification.Announcement(L10n.composer_drafting()).post()
                }
            case let .failed(failure, remedy):
                VStack(alignment: .leading, spacing: 6) {
                    Label(AiWords.failure(failure), systemImage: "exclamationmark.triangle")
                        .font(.callout)
                        .foregroundStyle(.orange)
                        .fixedSize(horizontal: false, vertical: true)
                    if let remedy {
                        ControlButton(title: AiWords.remedy(remedy.remedy), control: remedy.control) {
                            takeRemedy(remedy.remedy)
                        }
                        .buttonStyle(.bordered)
                    }
                }
            case let .drafted(summary, checklist):
                ReplyDraftCard(summary: summary, checklist: checklist) { item, done in
                    replyDrafts?.change(.tick(composition: composition, index: item, done: done))
                }
            case .unavailable, .ready:
                // A reason the person cannot act on is said here; one they can is in the input.
                if let held = record.heldBy, held.failure == .messageGone {
                    Text(AiWords.failure(held.failure))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    /// Following the record: read again on its signals and when From changes, the editor running
    /// each draft, and the questions about replacing and about Send.
    var replyDraftFollowing: ReplyDraftFollowing {
        ReplyDraftFollowing(
            verbs: replyDrafts, composition: composition, from: resolvedFrom, editor: editor,
            ui: $replyDraft, draft: { draft(replace: $0) }, send: prepareAndSend
        )
    }

    /// Send, asking first while the checklist's question is due. The core refuses the submit
    /// meanwhile as well, so this is the question, not the gate.
    func requestSend() {
        if replyDraft.record?.sendQuestion != nil {
            replyDraft.askingSend = true
        } else {
            prepareAndSend()
        }
    }

    private func openDraftInput() {
        replyDraft.line = replyDraft.record?.instruction ?? ""
        replyDraft.inputShown = true
    }

    /// Draft, or Return in the line: asked first while the person's text would be replaced.
    private func pressDraft() {
        if replyDraft.record?.askBeforeReplacing == true {
            replyDraft.inputShown = false
            replyDraft.askingReplace = true
        } else {
            draft(replace: false)
        }
    }

    private func draft(replace: Bool) {
        replyDraft.inputShown = false
        replyDrafts?.change(.draft(
            composition: composition, from: resolvedFrom, instruction: replyDraft.line,
            language: ReplyDraftWords.language, replace: replace))
    }

    /// Try again opens the input in place; anything else is opened by the app, the draft kept.
    private func takeRemedy(_ remedy: AiRemedy) {
        replyDraft.inputShown = false
        switch ComposerRemedyRoute(remedy) {
        case .input: openDraftInput()
        case .elsewhere: replyDrafts?.remedy(remedy)
        }
    }
}

/// Reads a composer's record again whenever it may have moved, hands the editor's messages to the
/// core, runs each draft in the editor, and asks the questions the record raises.
struct ReplyDraftFollowing: ViewModifier {
    let verbs: ComposerReplyDrafts?
    let composition: String
    let from: String?
    let editor: RichComposerEditor
    @Binding var ui: ReplyDraftUI
    let draft: (_ replace: Bool) -> Void
    let send: () -> Void

    func body(content: Content) -> some View {
        content
            .onAppear {
                guard let verbs else { return }
                let composition = composition
                editor.hostChannel.onMessage = { verbs.report(composition, $0) }
                read()
            }
            .onChange(of: verbs?.version) { _, _ in read() }
            .onChange(of: from) { _, _ in read() }
            .alert(L10n.composer_draft_replace_title(), isPresented: $ui.askingReplace) {
                Button(L10n.composer_draft_replace(), role: .destructive) { draft(true) }
                Button(L10n.action_cancel(), role: .cancel) {}
            } message: {
                Text(L10n.composer_draft_replace_message())
            }
            .alert(L10n.composer_draft_replace_title(), isPresented: arrived) {
                Button(L10n.composer_draft_replace(), role: .destructive) { answerArrived(true) }
                Button(L10n.action_cancel(), role: .cancel) { answerArrived(false) }
            } message: {
                Text(L10n.composer_draft_replace_message())
            }
            .alert(L10n.composer_send_open_title(), isPresented: $ui.askingSend) {
                Button(L10n.composer_send_anyway()) {
                    verbs?.change(.sendQuestionAnswered(composition: composition))
                    send()
                }
                Button(L10n.composer_keep_editing(), role: .cancel) {
                    verbs?.change(.sendQuestionAnswered(composition: composition))
                }
            } message: {
                if let question = ui.record?.sendQuestion {
                    Text(ReplyDraftWords.sendQuestion(question))
                }
            }
    }

    /// Up while a draft that arrived waits for the question; each answer is the core's to apply.
    private var arrived: Binding<Bool> {
        Binding(get: { ui.record?.replaceArrived == true }, set: { _ in })
    }

    private func answerArrived(_ replace: Bool) {
        verbs?.change(.answerArrived(composition: composition, replace: replace))
    }

    private func read() {
        ui.record = verbs?.read(composition, from)
        editor.followDraft(ui.record?.editorScript)
    }
}
