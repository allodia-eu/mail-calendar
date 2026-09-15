// The desktop's extra windows: what identifies one, and the drafts the composer windows are for
// (`docs/reading-window.md`). macOS only; nothing else in this app opens a second window.

#if os(macOS)
import SwiftUI

/// The scene ids the two extra window groups are registered under, and which `openWindow` names.
public enum DesktopWindow {
    public static let reading = "reading"
    public static let composer = "composer"
}

/// Which message a reading window is for.
///
/// Identity is the message, which is what makes double-clicking a row that is already open bring
/// its window forward instead of stacking a second one on the first: SwiftUI keeps one window per
/// distinct value, so the rule costs nothing to enforce.
public struct ReadingWindowID: Codable, Hashable {
    /// The account that owns the message (the row's own account).
    public let account: String
    /// The message's provider key, unique within that account.
    public let key: String
}

/// Which draft a composer window is for.
///
/// A minted id rather than the message being replied to: two replies to one message are two
/// drafts, and a new message is a draft with no message behind it at all. The draft itself stays
/// in `DesktopDrafts`, because a `ComposeContext` carries staged attachment handles and a
/// quoted-original seed, neither of which belongs in a value the window system persists.
public struct ComposerWindowID: Codable, Hashable {
    /// The draft's id, minted as the window is asked for.
    public let id: UUID
}

/// The drafts the open composer windows are showing, and the probe each reports its edits on.
///
/// Held beside the model rather than in it: a draft in a window is host state, the model has no
/// use for it, and the core is not told about a message until it is sent. One object shared by
/// every scene, so the window that opens a draft and the window that renders it are talking about
/// the same one.
@MainActor
@Observable
final class DesktopDrafts {
    /// What each open composer window is writing, keyed by the window's id.
    private(set) var contexts: [UUID: ComposeContext] = [:]

    /// Registers a draft and answers the id its window opens under.
    func open(_ context: ComposeContext) -> ComposerWindowID {
        let id = UUID()
        contexts[id] = context
        return ComposerWindowID(id: id)
    }

    /// The draft is finished with: sent, cancelled, or its window closed.
    func close(_ id: UUID) {
        contexts[id] = nil
    }

    /// Every open composer window, for the sweep that follows the main window closing.
    var openWindows: [ComposerWindowID] { contexts.keys.map(ComposerWindowID.init) }
}

extension View {
    /// Closes the reading and composer windows when the window this is attached to goes.
    ///
    /// The mailbox is what those windows were opened out of, and leaving them behind would leave
    /// the app running as a scatter of message windows with no way back to the list
    /// (`docs/reading-window.md`). Attached to the main window's content, so it fires whether the
    /// user closed that window or quit the app.
    public func closesItsWindows(session: AppSession) -> some View {
        modifier(ClosesItsWindows(model: session.model, drafts: session.drafts))
    }
}

private struct ClosesItsWindows: ViewModifier {
    let model: MailboxModel
    let drafts: DesktopDrafts

    @Environment(\.dismissWindow) private var dismissWindow

    func body(content: Content) -> some View {
        content.onDisappear {
            for reader in model.readingWindows.values {
                dismissWindow(
                    id: DesktopWindow.reading,
                    value: ReadingWindowID(
                        account: reader.message.account, key: reader.message.key
                    )
                )
            }
            // Belt as well as braces: each window's own close frees its slot, but a dismissal
            // the system does not deliver would otherwise leave a body in the core with nobody
            // left to read it.
            model.closeAllReadingWindows()
            for window in drafts.openWindows {
                dismissWindow(id: DesktopWindow.composer, value: window)
            }
        }
    }
}
#endif
