// Swipe-with-undo for the message list: the state machine a completed swipe runs through, and the
// labels the toast and the settings screen render. The Android client's SwipeUndo.kt is the same
// machine, keep the two in step.
//
// Delete and Archive are DEFERRED: the row hides locally the moment you swipe, but no intent is
// dispatched until the undo window closes. Undo therefore cancels the action outright, nothing
// ever reached the server, so there is no "un-move" to get wrong (an IMAP move mints a new UID, so
// the key we hold would be dead anyway). Star is different: it isn't destructive and the row stays
// in place, so it applies immediately and Undo un-stars.
//
// [SwipeUndoController] owns the decisions and nothing else: it returns the dispatch each step
// resolves to rather than reaching for the model, so the commit/revert/supersede rules can be
// exercised without a core.

import MailcalBindings
import SwiftUI

/// How long the undo toast stays up before a deferred Delete/Archive is dispatched.
let swipeUndoWindow: Duration = .seconds(4)

/// How long a committed Delete/Archive stays hidden after its intent is dispatched. The core hides
/// the row itself (an optimistic removal it publishes before any network round-trip), so this only
/// has to outlast one snapshot hop. It matters when the core's edit is REJECTED: the core then
/// restores the row, and un-hiding here lets it reappear instead of staying invisible until the
/// app restarts.
let swipeCommitHideGrace: Duration = .seconds(4)

/// The identity a hidden row is tracked under. The account is part of it because a provider key is
/// unique only WITHIN an account, so the unified inbox can show two rows with the same key.
func swipeRowKey(_ account: String, _ key: String) -> String { "\(account):\(key)" }

/// A swipe waiting out its undo window. `id` makes every swipe distinct even when the same message
/// is swiped the same way twice, so the view's undo-window task re-keys on the second swipe.
struct PendingSwipe: Equatable, Identifiable {
    let id: Int
    let account: String
    let key: String
    let action: SwipeActionKind

    /// Matches the `hiddenRowKeys` entries the list filters on.
    var rowKey: String { swipeRowKey(account, key) }

    /// Star toggles a flag in place, the row does not leave the list, so hiding it would be a lie.
    var hidesRow: Bool { action != .star }
}

/// The dispatch one step of the machine resolves to. The controller decides; the view applies it
/// through `MailboxModel`.
enum SwipeEffect: Equatable {
    case none
    case delete(account: String, key: String)
    case archive(account: String, key: String)
    case setFlagged(account: String, key: String, flagged: Bool)
}

/// Owns what a swipe does and when. Holds published state, so a view can read `pending` and
/// `hiddenRowKeys` directly, but the rules below are the interesting part and they run without a UI.
///
/// The rules:
/// - A Delete/Archive swipe hides the row and dispatches **nothing**; `commit` dispatches, `revert`
///   throws the action away.
/// - A Star swipe dispatches immediately (the row stays, so a delayed star would look broken);
///   `commit` is then a no-op and `revert` un-stars.
/// - `commit`/`revert` act only on the swipe that still owns the undo window (see `isCurrent`);
///   a stale one is dropped rather than dispatched.
@MainActor
final class SwipeUndoController: ObservableObject {
    /// The swipe currently inside its undo window, or `nil`.
    @Published private(set) var pending: PendingSwipe?
    /// Rows hidden while their swipe is pending (or briefly after it commits).
    @Published private(set) var hiddenRowKeys: Set<String> = []

    private var counter = 0

    /// Records a completed swipe, applying Star at once and hiding the row for Delete/Archive.
    /// The caller must first settle any still-pending swipe (see `ContentView.performSwipe`).
    func onSwipe(account: String, key: String, action: SwipeActionKind) -> SwipeEffect {
        counter += 1
        let swipe = PendingSwipe(id: counter, account: account, key: key, action: action)
        pending = swipe
        guard swipe.hidesRow else {
            return .setFlagged(account: account, key: key, flagged: true)
        }
        hiddenRowKeys.insert(swipe.rowKey)
        return .none
    }

    /// The undo window closed without an Undo: dispatch the deferred action.
    func commit(_ swipe: PendingSwipe) -> SwipeEffect {
        guard isCurrent(swipe) else { return .none }
        defer { pending = nil }
        switch swipe.action {
        case .delete: return .delete(account: swipe.account, key: swipe.key)
        case .archive: return .archive(account: swipe.account, key: swipe.key)
        // Star was applied the moment the row was swiped; committing is a no-op.
        case .star: return .none
        }
    }

    /// The user tapped Undo.
    func revert(_ swipe: PendingSwipe) -> SwipeEffect {
        guard isCurrent(swipe) else { return .none }
        defer { pending = nil }
        switch swipe.action {
        case .delete, .archive:
            // Nothing was dispatched: just put the row back.
            releaseHide(swipe)
            return .none
        case .star:
            return .setFlagged(account: swipe.account, key: swipe.key, flagged: false)
        }
    }

    /// Whether `swipe` still owns the undo window. A commit/revert can arrive for a swipe that is
    /// no longer current in two ways, and BOTH must be dropped rather than dispatched:
    /// - a newer swipe superseded it (the newer one owns `pending` now), and
    /// - the user tapped Undo while its window task was already awake, the task is cancelled a
    ///   moment later, so without this guard the reverted swipe would still dispatch its Delete.
    func isCurrent(_ swipe: PendingSwipe) -> Bool { pending?.id == swipe.id }

    /// Stops hiding `swipe`'s row. Called by `revert` straight away, and by the view a
    /// `swipeCommitHideGrace` after a commit, by then the core has published a snapshot without
    /// the row, so this is a no-op unless the core *rejected* the edit and restored it.
    func releaseHide(_ swipe: PendingSwipe) {
        hiddenRowKeys.remove(swipe.rowKey)
    }

    /// Drops the rows a swipe is hiding.
    func visibleRows(_ rows: [SnapshotRow]) -> [SnapshotRow] {
        guard !hiddenRowKeys.isEmpty else { return rows }
        return rows.filter { !isHidden($0) }
    }

    /// Whether `row` is currently hidden by a pending or just-committed swipe.
    func isHidden(_ row: SnapshotRow) -> Bool {
        guard case .flat(let message) = row else { return false }
        return hiddenRowKeys.contains(swipeRowKey(message.account, message.key))
    }
}

/// The toast text for a completed swipe, past tense, because by the time it shows, the row has
/// already left the list (or been starred).
func swipeDoneLabel(_ action: SwipeActionKind) -> String {
    switch action {
    case .delete: return L10n.swipe_done_delete()
    case .archive: return L10n.swipe_done_archive()
    case .star: return L10n.swipe_done_star()
    }
}

/// The settings (and swipe-button) label for a swipe action.
func swipeActionLabel(_ action: SwipeActionKind) -> String {
    switch action {
    case .delete: return L10n.swipe_action_delete()
    case .archive: return L10n.swipe_action_archive()
    case .star: return L10n.swipe_action_star()
    }
}

/// The SF Symbol for a swipe action, on its swipe button and in the settings picker.
func swipeActionSymbol(_ action: SwipeActionKind) -> String {
    switch action {
    case .delete: return "trash"
    case .archive: return "archivebox"
    case .star: return "star"
    }
}

/// The swipe-button tint. Delete takes the destructive role's own red, so it is asked for here
/// only to colour the settings picker's icon.
func swipeActionTint(_ action: SwipeActionKind) -> Color {
    switch action {
    case .delete: return .red
    case .archive: return .indigo
    case .star: return .orange
    }
}

/// The actions a direction can be bound to, in the order the settings picker lists them.
/// The generated enum is not `CaseIterable`, so the order lives here.
let swipeActionKinds: [SwipeActionKind] = [.delete, .archive, .star]
