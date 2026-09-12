// The detached composer window: one draft, in a window of its own (`docs/reading-window.md`).
//
// The composer is `ComposeHost`, the same one the detail column and the iOS cover mount, so a
// reply raised in a window is seeded, signed, submitted and cancelled by the paths that already
// exist. All this file adds is which window the draft belongs to and when to forget it.

#if os(macOS)
import SwiftUI

public struct ComposerWindow: View {
    private let model: MailboxModel
    private let drafts: DesktopDrafts
    private let window: ComposerWindowID

    @Environment(\.dismiss) private var dismiss

    public init(session: AppSession, window: ComposerWindowID) {
        self.model = session.model
        self.drafts = session.drafts
        self.window = window
    }

    public var body: some View {
        Group {
            if let context = drafts.contexts[window.id] {
                ComposeHost(model: model, context: context) {
                    // Sent or cancelled. Only the window is closed here; the draft is forgotten in
                    // `onDisappear`, which is the one place that runs however the window went,
                    // including the red button. Clearing it first would flip this view into the
                    // branch below while the window was still up, and leave an empty one behind.
                    dismiss()
                }
                .navigationTitle(context.windowTitle)
            } else {
                // A window with no draft behind it: nothing to show, and an empty composer
                // claiming to be a reply is worse than no window at all.
                Color.clear.onAppear { dismiss() }
            }
        }
        .frame(minWidth: 620, minHeight: 460)
        .onDisappear { drafts.close(window.id) }
    }
}
#endif
