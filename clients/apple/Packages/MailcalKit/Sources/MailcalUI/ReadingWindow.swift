// The detached reading window: one message, in a window of its own (`docs/reading-window.md`).
//
// Everything the window draws is `ReadingView`, the same view the pane draws, reading a slot of
// its own in the core. What this file adds is the three things only a window has: where its body
// comes from, what its action row does when there is no pane to fall back to, and telling the core
// to drop the body when the window goes.

#if os(macOS)
import MailcalBindings
import SwiftUI

public struct ReadingWindow: View {
    private let model: MailboxModel
    private let drafts: DesktopDrafts
    private let window: ReadingWindowID

    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismiss) private var dismiss

    public init(session: AppSession, window: ReadingWindowID) {
        self.model = session.model
        self.drafts = session.drafts
        self.window = window
    }

    /// The core reader id this window's body arrives in.
    private var readerID: String {
        MailboxModel.readingWindowID(account: window.account, key: window.key)
    }

    public var body: some View {
        Group {
            if let opened = model.readingWindows[readerID] {
                ReadingView(
                    model: model,
                    message: opened.message,
                    source: .window(readerID),
                    onReply: { reply(to: opened, all: false) },
                    onReplyAll: { reply(to: opened, all: true) },
                    onForward: { forward(opened) },
                    // Archive and delete move the message out of the folder, so there is nothing
                    // left for this window to be about. The pane advances to the next message
                    // down (Mailcal.AutoAdvance.swift) because it is a place in a list; a window
                    // is one message, so it closes.
                    onArchive: {
                        model.archive(window.account, window.key)
                        dismiss()
                    },
                    onDelete: {
                        model.delete(window.account, window.key)
                        dismiss()
                    }
                )
                .navigationTitle(
                    opened.message.subject.isEmpty
                        ? L10n.mail_no_subject() : opened.message.subject
                )
            } else {
                // No entry means the window is not one this session opened: the system restored
                // it from a previous launch, where the header it was opened on did not survive.
                // Rather than draw a window with no subject and no sender, it closes and leaves
                // the message where the user can open it again.
                Color.clear.onAppear { dismiss() }
            }
        }
        .frame(minWidth: 520, minHeight: 400)
        // Both halves of the close: the host forgets the header, and the core drops the body it
        // was holding for this reader.
        .onDisappear { model.closeReadingWindow(readerID) }
    }

    /// Replying from a window opens a composer window, which is where a reply raised out of a
    /// window belongs: the shell's inline composer would put the draft in a different window from
    /// the message it answers, behind whatever the user has in front of them.
    private func reply(to opened: DetachedReading, all: Bool) {
        openDraft(
            model.replyDraft(
                account: window.account, key: window.key,
                subject: opened.message.subject, all: all,
                quotingFrom: opened.message, body: opened.body
            )
        )
    }

    private func forward(_ opened: DetachedReading) {
        Task { @MainActor in
            openDraft(
                await model.forwardDraft(
                    account: window.account, key: window.key,
                    subject: opened.message.subject,
                    quotingFrom: opened.message, body: opened.body
                )
            )
        }
    }

    private func openDraft(_ context: ComposeContext) {
        openWindow(id: DesktopWindow.composer, value: drafts.open(context))
    }
}
#endif
