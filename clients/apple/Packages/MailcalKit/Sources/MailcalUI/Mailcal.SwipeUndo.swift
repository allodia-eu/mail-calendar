// The shell's half of swipe-with-undo: it turns the decisions `SwipeUndoController` makes into
// intents on the model, runs the undo window, and renders the toast. Split from Mailcal.swift so
// the app shell stays under the repo line limit.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// The rows the list actually shows: everything the core projected, minus the ones a pending
    /// (or just-committed) swipe is hiding.
    var visibleRows: [SnapshotRow] { swipeUndo.visibleRows(model.rows) }

    /// A completed swipe on a row. An earlier swipe still inside its undo window is committed
    /// first, a user swiping two messages in a row expects the first one to have happened.
    func performSwipe(_ account: String, _ key: String, _ action: SwipeActionKind) {
        if let previous = swipeUndo.pending { finishSwipe(previous, undone: false) }
        apply(swipeUndo.onSwipe(account: account, key: key, action: action))
    }

    /// Closes a swipe's undo window: dispatch the deferred action, or throw it away. A committed
    /// row is un-hidden after a grace period, so a core-*rejected* edit, which makes the core
    /// restore the row, doesn't leave it invisible until the app restarts.
    ///
    /// A swipe that no longer owns the window (undone, or superseded and already settled) is a
    /// no-op in the controller, and must not schedule an un-hide either, that would reveal a row
    /// the *newer* swipe is legitimately hiding.
    func finishSwipe(_ swipe: PendingSwipe, undone: Bool) {
        guard swipeUndo.isCurrent(swipe) else { return }
        apply(undone ? swipeUndo.revert(swipe) : swipeUndo.commit(swipe))
        guard !undone, swipe.hidesRow else { return }
        Task { @MainActor in
            try? await Task.sleep(for: swipeCommitHideGrace)
            swipeUndo.releaseHide(swipe)
        }
    }

    /// Applies the controller's decision to the core.
    private func apply(_ effect: SwipeEffect) {
        switch effect {
        case .none:
            break
        case let .delete(account, key):
            model.delete(account, key)
        case let .archive(account, key):
            model.archive(account, key)
        case let .setFlagged(account, key, flagged):
            model.setFlagged(account, key, flagged)
        }
    }

    /// Runs the pending swipe's undo window. Keyed on the swipe, so a second swipe cancels this
    /// and `performSwipe` commits the first. The one gap (as on Android): killing the app inside
    /// the window loses a deferred action.
    func runUndoWindow() async {
        guard let swipe = swipeUndo.pending else { return }
        try? await Task.sleep(for: swipeUndoWindow)
        guard !Task.isCancelled else { return }
        finishSwipe(swipe, undone: false)
    }

    /// The swipe confirmation with its Undo, over the bottom of the list. Tapping Undo closes the
    /// window early: for a Delete/Archive nothing was ever dispatched, so the row simply returns.
    @ViewBuilder
    var swipeUndoToast: some View {
        if let swipe = swipeUndo.pending {
            HStack(spacing: 14) {
                Text(swipeDoneLabel(swipe.action)).font(.callout)
                Button(L10n.action_undo()) { finishSwipe(swipe, undone: true) }
                    .buttonStyle(.borderless)
                    .font(.callout.weight(.semibold))
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.thinMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(.quaternary))
            .shadow(radius: 6, y: 2)
            .padding(.bottom, 16)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .animation(.easeInOut(duration: 0.2), value: swipe)
        }
    }
}
