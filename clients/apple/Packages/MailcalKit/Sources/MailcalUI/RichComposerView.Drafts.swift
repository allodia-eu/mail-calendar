// Keeping the composer's message on the server: the autosave timer, the Save button's action and
// the hint under the editor (`docs/drafts.md`).
//
// A child of RichComposerView.swift, whose `RichComposeView` this extends; its own file because
// that one is at the 500-line limit.

import Foundation
import MailcalBindings
import SwiftUI

/// The four core verbs a composer keeps its draft with, each naming the composition the composer
/// minted for itself.
///
/// Passed as a value rather than the model, as `ComposerSignatures` is, so `RichComposeView` stays
/// free of the model; `nil` turns draft saving off entirely, which is what a preview and a
/// screenshot run want.
struct ComposerDrafts {
    /// Stores what the composer holds, superseding this composition's previous save.
    let save: (String, Recipients, String, String, [ComposerFileAttachment], String?) -> Void
    /// Removes the stored copy from the server and forgets the composition.
    let discard: (String) -> Void
    /// Forgets the composition, leaving the stored draft in Drafts: what closing a composer means.
    let close: (String) -> Void
    /// How this composition's most recent save ended.
    let status: (String) -> DraftStatus
    /// The model's count of `Surface::DraftStatus` signals. The signal names no composition, so a
    /// composer watches this and then asks for its own state.
    let version: Int
}

/// The composer's whole draft lifetime, as one modifier both hosts apply: the idle timer that
/// stores it, the hint that follows the save, and forgetting the composition on the way out.
///
/// A modifier rather than lines in each body for the reason `ComposerDropModifier` is one: the
/// macOS pane and the iPhone cover must not be able to come to behave differently, and only one of
/// them is easy to run.
struct ComposerDraftSavingModifier: ViewModifier {
    let drafts: ComposerDrafts?
    /// The composition every call names, minted by the composer and kept until it closes.
    let composition: String
    /// How many changes the composer has seen, header and body alike. Every one restarts the idle
    /// interval; the value itself is never read for anything else.
    @Binding var changes: Int
    /// The most recent save's state, for the hint.
    @Binding var status: DraftStatus
    let editor: RichComposerEditor
    let save: () -> Void

    func body(content: Content) -> some View {
        content
            .task { await watchTheEditor() }
            .task(id: changes) { await saveWhenIdle() }
            .onChange(of: drafts?.version) { _, _ in
                status = drafts.map { $0.status(composition) } ?? .idle
            }
            // However the composer went, sent, cancelled or dismissed. Without it the core holds
            // a record per composer for the life of the process, and the stored draft would be
            // superseded by whatever the next composer to take this id wrote.
            .onDisappear { drafts?.close(composition) }
    }

    /// Watches the editor for changes this host cannot otherwise see.
    ///
    /// Everything in the header is SwiftUI state and raises its own `onChange`; the message body
    /// is a web view, and the page has no channel back to here. So the bundle counts its own
    /// mutations and this samples the count, calling the composer changed when it moves.
    ///
    /// Sampled at a third of the idle interval, which is what the lag costs: a draft reaches the
    /// server between one and one-and-a-third intervals after the last keystroke, and never
    /// during typing.
    private func watchTheEditor() async {
        guard drafts != nil else { return }
        let sample = Duration.seconds(max(1, draftAutosaveIdleSeconds() / 3))
        var seen = await editor.revision()
        while !Task.isCancelled {
            try? await Task.sleep(for: sample)
            guard !Task.isCancelled else { return }
            let now = await editor.revision()
            guard now != seen else { continue }
            seen = now
            changes &+= 1
        }
    }

    /// Stores the draft once the composer has been idle for the core's interval.
    ///
    /// The whole timer is a `Task.sleep` under a `task(id:)`: every change restarts the task,
    /// which cancels the sleep in flight, so the trigger is the pause and not the clock. A
    /// composer nobody has touched has nothing to save and starts no timer.
    private func saveWhenIdle() async {
        guard drafts != nil, changes > 0 else { return }
        try? await Task.sleep(for: .seconds(draftAutosaveIdleSeconds()))
        guard !Task.isCancelled else { return }
        save()
    }
}

extension RichComposeView {
    /// Everything the composer's draft needs: the idle timer, the hint's refresh, and forgetting
    /// the composition on the way out. One modifier on both hosts, so the macOS pane and the
    /// iPhone cover cannot come to save differently.
    var draftSaving: ComposerDraftSavingModifier {
        ComposerDraftSavingModifier(
            drafts: drafts,
            composition: composition,
            changes: $draftChanges,
            status: $draftStatus,
            editor: editor,
            save: saveDraftNow
        )
    }

    /// Stores the draft now, whatever the idle timer is doing.
    ///
    /// Both triggers land here and the core cannot tell them apart, which is deliberate: pressing
    /// Save on an unchanged draft reaches no server, exactly as an idle timer firing on an
    /// untouched composer does.
    ///
    /// A document the editor cannot render is logged and dropped rather than shown: saving is
    /// never something the user waits for, and never something a composer refuses to be dismissed
    /// over. A save the *server* refuses is reported, through the hint.
    func saveDraftNow() {
        guard let drafts else { return }
        editor.documentJSON { result in
            guard case let .success(documentJson) = result else {
                print("[Mailcal] draft save skipped: the composer document could not be read")
                return
            }
            drafts.save(
                composition,
                Recipients(to: to, cc: cc, bcc: bcc),
                subject,
                documentJson,
                attachments.map(\.composerFile),
                resolvedFrom
            )
        }
    }

    /// Removes this composer's stored draft, for the shell's Discard button, or `nil` when this
    /// composer keeps no draft and so has nothing on the server to take away.
    ///
    /// Handed to the probe rather than reached for: the shell knows a draft is up and not which
    /// composition it is.
    var discardStoredDraft: (() -> Void)? {
        drafts.map { drafts in { drafts.discard(composition) } }
    }

    /// The quiet line under the editor, or `nil` when there is nothing to say.
    ///
    /// A hint and never a gate: no state here stops the composer being closed, and none of it is
    /// worth a modal. A composer that has saved nothing says nothing.
    var draftHint: String? {
        switch draftStatus {
        case .idle: return nil
        case .saving: return L10n.compose_draft_saving()
        case .saved: return L10n.compose_draft_saved()
        case .queued: return L10n.compose_draft_queued()
        case .failed: return L10n.compose_draft_failed()
        }
    }

    /// Whether the hint is reporting something that did not work, the one state it is drawn in
    /// the error colour.
    var draftHintFailed: Bool {
        if case .failed = draftStatus { return true }
        return false
    }
}
