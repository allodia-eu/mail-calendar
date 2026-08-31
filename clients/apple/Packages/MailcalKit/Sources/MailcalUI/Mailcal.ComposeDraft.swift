// The unsent-draft guard for the macOS inline composer.
//
// On macOS the composer no longer opens as a window-sheet: it REPLACES the reading pane, so the
// sidebar and the message list stay live and clickable while you write. That makes a click on
// another message reachable for the first time, and it would silently throw the draft away. So it
// asks first: Discard, or Keep editing.
//
// iPhone and iPad keep the full-screen composer, where no row is clickable behind it, so none of
// this applies there, the guard short-circuits.

import MailcalBindings
import SwiftUI

/// Whether the composer in the detail column has anything in it worth keeping.
///
/// `RichComposeView` is a struct that owns its field state privately, so the shell, the thing that
/// has to ask "discard this draft?" before handing the column to another message, cannot see
/// inside it. This is the narrow channel between the two: the composer reports header edits
/// eagerly, and hands over a closure the shell can await to ask the editor whether its document
/// still matches the quoted original it opened with.
@MainActor
final class ComposeDraftProbe: ObservableObject {
    /// Set the moment the user edits To/Cc/Bcc/Subject, or attaches a file. Needs no round-trip.
    var headersEdited = false

    /// Asks the hosted editor whether its document differs from the seed it opened with.
    var bodyEdited: (() async -> Bool)?

    /// Whether anything has been written since the composer opened.
    ///
    /// A reply that merely carries its quoted original is **not** dirty: the seeded document is the
    /// baseline, so the user has to actually write something (or restyle the quote) before we
    /// interrupt them. Opening a reply and immediately clicking another message must not prompt.
    func isDirty() async -> Bool {
        if headersEdited { return true }
        return await bodyEdited?() ?? false
    }

    /// Forgets the previous draft. Called as each composer opens.
    func reset() {
        headersEdited = false
        bodyEdited = nil
    }
}

/// The header state the draft probe watches, one `Equatable` value, so a single `onChange` covers
/// every field instead of one per field.
private struct DraftHeaders: Equatable {
    let to: String
    let cc: String
    let bcc: String
    let subject: String
    let attachments: Int
}

extension View {
    /// Reports the composer's edits to the shell's draft probe, so the "Discard draft?" prompt can
    /// tell a written draft from an untouched one.
    ///
    /// Header edits are recorded eagerly (they need no round-trip). The body is asked for on demand
    /// instead, reading the editor document means a hop into its WebView, which is not worth doing
    /// on every keystroke when the answer is only ever needed at the moment the user clicks away.
    /// The recipient pre-fill of a reply arrives as the field's *initial* value, so it raises no
    /// change and does not make the draft dirty.
    func composeDraftTracking(
        probe: ComposeDraftProbe?,
        editor: RichComposerEditor,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        attachments: Int
    ) -> some View {
        onAppear {
            probe?.reset()
            probe?.bodyEdited = { await editor.bodyChangedFromSeed() }
        }
        .onDisappear { probe?.reset() }
        .onChange(
            of: DraftHeaders(to: to, cc: cc, bcc: bcc, subject: subject, attachments: attachments)
        ) { _, _ in
            probe?.headersEdited = true
        }
    }
}

extension ContentView {
    /// Performs `open`, unless a draft is up with something written in it, in which case it asks
    /// first and defers `open` until the user chooses. `Discard` drops the draft and runs it;
    /// `Keep editing` abandons it and leaves the composer alone.
    ///
    /// A clean draft (nothing typed) is dropped without a prompt: there is nothing to lose, and
    /// stopping the user to say so would be noise.
    func openGuardingDraft(_ open: @escaping () -> Void) {
        #if os(macOS)
        guard compose != nil else {
            open()
            return
        }
        Task { @MainActor in
            if await draftProbe.isDirty() {
                pendingOpen = open
                confirmingDiscard = true
            } else {
                compose = nil
                open()
            }
        }
        #else
        // iOS/iPadOS: the composer is a full-screen cover, so there is no row behind it to click.
        open()
        #endif
    }

    /// Opens an assistant's draft in the composer, unsent (docs/mcp.md).
    ///
    /// Behind the same discard guard a message click uses. An assistant asking to open a draft
    /// must not be able to throw away a half-written message the user is in the middle of, it
    /// arrives from another process, unprompted, and could arrive at any moment.
    func openDraft(_ request: AgentDraftRequest) {
        openGuardingDraft { compose = .agentDraft(request) }
    }
}

/// The "Discard draft?" confirmation, attached to the shell. Only macOS can raise it, it fires
/// when a click lands on another message while an inline draft has something written in it.
///
/// `Discard` drops the draft and runs the open that was deferred; `Keep editing` drops the deferred
/// open instead and leaves the composer exactly as it was.
struct DiscardDraftDialog: ViewModifier {
    @Binding var isPresented: Bool
    @Binding var compose: ComposeContext?
    @Binding var pendingOpen: (() -> Void)?

    func body(content: Content) -> some View {
        content// An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
// popover DROPS the `.cancel`-role button, so this read as one destructive button with no
// way out. See the remove-account alert in Mailcal.swift for the full note.
.alert(
            L10n.compose_discard_title(),
            isPresented: $isPresented
        ) {
            Button(L10n.action_discard(), role: .destructive) {
                compose = nil
                let open = pendingOpen
                pendingOpen = nil
                open?()
            }
            Button(L10n.action_keep_editing(), role: .cancel) {
                pendingOpen = nil
            }
        } message: {
            Text(L10n.compose_discard_message())
        }
    }
}
