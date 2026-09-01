// Mail row rendering and row-driven actions for the shared Apple client. Split from
// Mailcal.swift so the app shell stays under the repo line limit.

import SwiftUI
import MailcalBindings

extension ContentView {
    /// Opens the reply (or reply-all) composer for a message, computing the core's suggested
    /// recipients once and carrying them in the compose context.
    func beginReply(_ account: String, _ key: String, all: Bool) {
        let prefill = model.replyRecipients(account, key, all)
        let seed = quoteSeed(account, key, isForward: false)
        compose = all
            ? .replyAll(account: account, key: key, to: prefill?.to ?? "", cc: prefill?.cc ?? "",
                        quote: seed.quote, quoteStyle: seed.style)
            : .reply(account: account, key: key, to: prefill?.to ?? "", cc: prefill?.cc ?? "",
                     quote: seed.quote, quoteStyle: seed.style)
    }

    /// Opens the forward composer, seeding the quoted original the same way as a reply.
    func beginForward(_ account: String, _ key: String) {
        let seed = quoteSeed(account, key, isForward: true)
        compose = .forward(account: account, key: key, quote: seed.quote, quoteStyle: seed.style)
    }

    /// The quoted-original seed for a reply/forward of `(account, key)`, plus the default style.
    /// A showcase reply to the designated message also carries sample body text, so the store
    /// screenshot shows a written reply rather than an empty composer.
    private func quoteSeed(
        _ account: String,
        _ key: String,
        isForward: Bool
    ) -> (quote: String?, style: QuoteStyleKind) {
        let style = model.quoteSettingsNow().style
        guard let opened = openedMessage, opened.account == account, opened.key == key else {
            return (nil, style)
        }
        let quote = ComposerQuote.seedJSON(
            style: style,
            message: opened,
            reading: model.reading,
            isForward: isForward,
            initialText: isForward ? nil : ShowcaseMode.replyText(account: account, key: key)
        )
        return (quote, style)
    }

    /// Opens a message for reading: records the header and asks the core to fetch its body.
    ///
    /// Guarded: on macOS the composer lives in the detail column, so opening a message would drop
    /// an unsent draft. `openGuardingDraft` asks first when there is something written to lose
    /// (Mailcal.ComposeDraft.swift); on iPhone/iPad it just runs the open.
    func open(_ message: FlatRow) {
        openGuardingDraft { openNow(message) }
    }

    private func openNow(_ message: FlatRow) {
        openNow(opened(message))
    }

    /// Opens a conversation: expands the thread inline in the list (showing its whole
    /// conversation as sub-rows) and auto-opens its representative message in the reading pane:
    /// the latest **in-scope** message (`latestKey`, the one the row summarises), so a Sent reply
    /// filed elsewhere doesn't steal focus. Tapping again collapses it and leaves the pane as-is.
    ///
    /// The whole thing is guarded, expansion included: "Keep editing" should leave the list exactly
    /// as it was, not expand the thread the user has just been talked out of opening.
    func open(_ thread: ThreadRow) {
        openGuardingDraft {
            let key = threadKey(thread)
            if expandedThreads.contains(key) {
                expandedThreads.remove(key)
                return
            }
            expandedThreads.insert(key)
            let representative = thread.messages.first(where: { $0.key == thread.latestKey })
                ?? thread.messages.first
            if let representative {
                openThreadMessageNow(thread, representative)
            }
        }
    }

    /// Opens one message of a conversation (a sub-row) in the reading pane.
    func openThreadMessage(_ thread: ThreadRow, _ message: ThreadMessage) {
        openGuardingDraft { openThreadMessageNow(thread, message) }
    }

    // The unguarded open, so `open(_ thread:)`, already inside the guard, doesn't ask twice.
    private func openThreadMessageNow(_ thread: ThreadRow, _ message: ThreadMessage) {
        openNow(opened(message, subject: thread.subject))
    }

    /// The expansion key for a thread (its account-scoped id).
    func threadKey(_ thread: ThreadRow) -> String { "\(thread.account)/\(thread.threadId)" }

    /// Whether `(account, key)` is the message currently open in the reading pane.
    func isOpenMessage(_ account: String, _ key: String) -> Bool {
        openedMessage.map { $0.account == account && $0.key == key } ?? false
    }

    /// The unread marker, on the layouts `docs/avatars.md` gives one to: the compact phone list
    /// has none, because bold subject and sender already carry unread and the width is better
    /// spent on the text.
    ///
    /// **The size class must be read at the window level, which is why this lives on the view and
    /// not inside `UnreadDot`.** Each column of a `NavigationSplitView` reports its *own*
    /// horizontal size class, so an iPad's list column reads `.compact` while the window is
    /// regular, a dot that decided for itself would disappear on exactly the layout the rule
    /// keeps it for. Seen on an iPad Pro: reading pane on screen, no dot on an unread row.
    @ViewBuilder
    func unreadDot(_ unread: Bool) -> some View {
        if hasReadingPane {
            UnreadDot(unread: unread)
        }
    }

    @ViewBuilder
    func rowView(_ row: SnapshotRow) -> some View {
        // Each row is handed its own click rule rather than deciding for itself: what a click
        // means depends on the modifiers (macOS) or the mode (iPhone, iPad), and on where the row
        // sits, so a Shift-click knows what range it is extending (`docs/list-selection.md`).
        switch row {
        case .flat(let message):
            flatRow(message, click: { applySelectionClick(row) })
        case .thread(let thread):
            threadRow(thread, click: { applySelectionClick(row) })
        }
    }

    /// Whether `row` is one of the rows picked out to act on together.
    func isSelectedRow(_ row: SnapshotRow) -> Bool { selection.contains(row) }

    private func flatRow(_ message: FlatRow, click: @escaping () -> Bool) -> some View {
        HStack(spacing: 8) {
            // Alignment gutter: conversation rows put their expand/collapse chevron here
            // (Outlook-style). A flat row reserves the same width and leaves it empty, so the
            // leading glyph and the subject line up across flat and threaded rows.
            Color.clear.frame(width: 14)
            unreadDot(message.unread)
            AvatarView(avatar: message.avatar)
            VStack(alignment: .leading, spacing: 2) {
                // Subject AND sender carry the weight, not the glyph alone: an unread row has to be
                // findable while scanning the list, and a 22-point envelope in the gutter is not what
                // the eye lands on. The preview line stays regular, bolding all three makes an
                // unread mailbox one solid block, which distinguishes nothing.
                Text(message.subject.isEmpty ? L10n.mail_no_subject() : message.subject)
                    .lineLimit(1)
                    .fontWeight(message.unread ? .semibold : .regular)
                Text(message.from)
                    .font(.caption)
                    .fontWeight(message.unread ? .semibold : .regular)
                    .foregroundStyle(.secondary)
                // The provider's body-preview snippet (empty for IMAP until the engine supplies it).
                if !message.preview.isEmpty {
                    Text(message.preview)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer()
            if message.flagged {
                Image(systemName: "flag.fill").foregroundStyle(.orange)
            }
            if message.hasAttachment {
                Image(systemName: "paperclip").foregroundStyle(.secondary)
            }
            Text(relativeDate(message.date, in: model.activeZone))
                .font(.caption2).foregroundStyle(.secondary)
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
        // A modified click (or a tap in selection mode) is aimed at the selection alone: opening
        // as well would fetch and display a body for every row added to a twenty-row set.
        .onTapGesture { if click() { open(message) } }
        // Each direction runs exactly the one action the user configured (Settings → Reading),
        // with an undo toast as the net. A swipe *rightwards* reveals the leading edge, so that
        // edge carries `swipeSettings.right`; the trailing edge carries `.left`. Mark read/unread
        // and flag moved to the context menu, which is where the other row actions already live.
        .swipeActions(edge: .leading, allowsFullSwipe: true) {
            swipeButton(model.swipeSettings.right, message)
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            swipeButton(model.swipeSettings.left, message)
        }
        .contextMenu { flatRowMenu(message) }
    }

    /// The single button one swipe edge reveals: the configured action, dispatched through the
    /// undo window rather than straight at the core.
    @ViewBuilder
    private func swipeButton(_ action: SwipeActionKind, _ message: FlatRow) -> some View {
        Button(role: action == .delete ? .destructive : nil) {
            performSwipe(message.account, message.key, action)
        } label: {
            Label(swipeActionLabel(action), systemImage: swipeActionSymbol(action))
        }
        .tint(swipeActionTint(action))
    }

    @ViewBuilder
    private func flatRowMenu(_ message: FlatRow) -> some View {
        Button { open(message) } label: {
            Label(L10n.action_open(), systemImage: "envelope.open")
        }
        Divider()
        Button { beginReply(message.account, message.key, all: false) } label: {
            Label(L10n.action_reply(), systemImage: "arrowshape.turn.up.left")
        }
        Button { beginReply(message.account, message.key, all: true) } label: {
            Label(L10n.action_reply_all(), systemImage: "arrowshape.turn.up.left.2")
        }
        Button { beginForward(message.account, message.key) } label: {
            Label(L10n.action_forward(), systemImage: "arrowshape.turn.up.right")
        }
        Divider()
        Button { model.markRead(message.account, message.key, message.unread) } label: {
            Label(message.unread ? L10n.action_mark_read() : L10n.action_mark_unread(), systemImage: "envelope")
        }
        Button { model.setFlagged(message.account, message.key, true) } label: {
            Label(L10n.action_flag(), systemImage: "flag")
        }
        Button { model.setFlagged(message.account, message.key, false) } label: {
            Label(L10n.action_clear_flag(), systemImage: "flag.slash")
        }
        Divider()
        // Archive alongside Trash, so both destinations are on the menu (not only the swipe).
        Button { model.archive(message.account, message.key) } label: {
            Label(L10n.action_archive(), systemImage: "archivebox")
        }
        Button { model.delete(message.account, message.key) } label: {
            Label(L10n.action_move_to_trash(), systemImage: "trash")
        }
        Button(role: .destructive) {
            model.permanentlyDelete(message.account, message.key)
        } label: {
            Label(L10n.action_delete_permanently(), systemImage: "trash.slash")
        }
    }

    /// A conversation row: a tappable header (subject, message count, latest sender) that, when
    /// expanded, reveals the whole thread as indented sub-rows, every message on it, received
    /// and the account owner's own Sent replies alike (the core gathers them across folders).
    @ViewBuilder
    private func threadRow(_ thread: ThreadRow, click: @escaping () -> Bool) -> some View {
        let expanded = expandedThreads.contains(threadKey(thread))
        VStack(spacing: 0) {
            threadHeader(thread, expanded: expanded, click: click)
            if expanded {
                ForEach(thread.messages, id: \.key) { message in
                    Divider().padding(.leading, 30)
                    threadMessageRow(thread, message)
                }
            }
        }
    }

    /// The thread's summary header. A collapsed thread whose open message is in the reading pane
    /// is highlighted here (expanded, the open sub-row carries the highlight instead).
    private func threadHeader(
        _ thread: ThreadRow,
        expanded: Bool,
        click: @escaping () -> Bool
    ) -> some View {
        let active = !expanded && thread.messages.contains { isOpenMessage($0.account, $0.key) }
        // A conversation is unread if anything in it is: the header is a summary, and it may not
        // read as settled while it hides an unread reply. Its sub-rows carry their own weight.
        let unread = thread.unreadCount > 0
        return HStack(spacing: 8) {
            // The expand/collapse chevron sits in the same leading gutter a flat row reserves
            // (Outlook-style), so the conversation glyph and subject align with the flat rows.
            Image(systemName: expanded ? "chevron.down" : "chevron.right")
                .font(.caption).foregroundStyle(.secondary).frame(width: 14)
            unreadDot(unread)
            // The conversation's latest sender, which is who the row's other text names. The
            // chevron and the message-count badge already say this is a thread, so the glyph
            // that used to sit here said nothing the row did not.
            AvatarView(avatar: thread.avatar)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(thread.subject.isEmpty ? L10n.mail_no_subject() : thread.subject)
                        .lineLimit(1)
                        .fontWeight(unread ? .semibold : .regular)
                    if thread.messageCount > 1 {
                        Text("\(thread.messageCount)")
                            .font(.caption2)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(.quaternary, in: Capsule())
                    }
                }
                Text(thread.latestFrom)
                    .font(.caption)
                    .fontWeight(unread ? .semibold : .regular)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if thread.hasAttachment {
                Image(systemName: "paperclip").foregroundStyle(.secondary)
            }
            Text(relativeDate(thread.latestDate, in: model.activeZone))
                .font(.caption2).foregroundStyle(.secondary)
        }
        .padding(.vertical, 3)
        .background(active ? Color.accentColor.opacity(0.15) : Color.clear)
        .contentShape(Rectangle())
        .onTapGesture { if click() { open(thread) } }
        .contextMenu { threadMenu(thread) }
    }

    /// Right-click actions on a conversation. "Archive conversation" archives the received side
    /// only, the core leaves any Sent copies in Sent (they stay visible in the thread).
    @ViewBuilder
    private func threadMenu(_ thread: ThreadRow) -> some View {
        Button { archiveThread(thread) } label: {
            Label(L10n.thread_archive(), systemImage: "archivebox")
        }
    }

    /// Archives a conversation (received messages only) and tidies the UI: collapse it and, if
    /// the open message was part of it, clear the reading pane, its row is leaving the folder.
    private func archiveThread(_ thread: ThreadRow) {
        model.archiveThread(thread.account, thread.threadId)
        expandedThreads.remove(threadKey(thread))
        if let opened = openedMessage,
            thread.messages.contains(where: { $0.account == opened.account && $0.key == opened.key }) {
            clearOpenedMessage()
        }
    }

    /// One message of an expanded conversation: an indented sub-row (sender, Sent badge, preview,
    /// date), highlighted when it is the message open in the reading pane. Tapping opens it.
    private func threadMessageRow(_ thread: ThreadRow, _ message: ThreadMessage) -> some View {
        HStack(spacing: 8) {
            unreadDot(message.unread)
            AvatarView(avatar: message.avatar, diameter: 26)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(message.from.isEmpty ? L10n.mail_no_subject() : message.from)
                        .font(.subheadline)
                        .fontWeight(message.unread ? .semibold : .regular)
                        .lineLimit(1)
                    if message.outgoing {
                        Text(L10n.thread_sent())
                            .font(.caption2)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(.quaternary, in: Capsule())
                    }
                }
                if !message.preview.isEmpty {
                    Text(message.preview)
                        .font(.caption).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.tail)
                }
            }
            Spacer()
            if message.hasAttachment {
                Image(systemName: "paperclip").foregroundStyle(.secondary)
            }
            Text(relativeDate(message.date, in: model.activeZone))
                .font(.caption2).foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
        .padding(.leading, 30)
        .background(isOpenMessage(message.account, message.key) ? Color.accentColor.opacity(0.15) : Color.clear)
        .contentShape(Rectangle())
        .onTapGesture { openThreadMessage(thread, message) }
    }
}
