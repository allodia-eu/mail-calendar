// Mail row rendering and row-driven actions for the shared Apple client. Split from
// Mailcal.swift so the app shell stays under the repo line limit.

import SwiftUI
import MailcalBindings

extension ContentView {
    /// Opens the reply (or reply-all) composer for a message, in the shell's own composer: the
    /// detail column on macOS, `taskPresentation` on iOS.
    ///
    /// The core answers with the recipients, the subject and the quoted original, so a reply
    /// raised from a list row that was never opened quotes too.
    func beginReply(_ account: String, _ key: String, all: Bool) {
        let (origin, prefill) = replyOrigin(account: account, key: key, all: all)
        beginCompose(origin, prefill: prefill)
    }

    /// Opens the forward composer in the shell, holding the original's files, which the core has
    /// staged before it answers.
    func beginForward(_ account: String, _ key: String) {
        beginCompose(.forward(account: account, key: key))
    }

    /// Opens a message for reading: records the header and asks the core to fetch its body. In
    /// the Drafts folder it opens the composer the message was written in instead.
    ///
    /// On macOS the composer lives in the detail column, so opening a message takes the column
    /// from it; `openGuardingDraft` hands it over, and the composer keeps its draft in Drafts on
    /// the way out (Mailcal.ComposeDraft.swift). On iPhone/iPad it just runs the open.
    func open(_ message: FlatRow) {
        openGuardingDraft {
            resumeOrRead(message.account, message.key) { openNow(message) }
        }
    }

    private func openNow(_ message: FlatRow) {
        openNow(opened(message))
    }

    /// Opens a Drafts-folder row back into its composer, and every other row for reading.
    ///
    /// Told by the **folder**, not by the row: the core answers per message but the list does not
    /// carry it, so a draft met in a search result or in another folder's thread opens read-only
    /// (`docs/drafts.md`, known gaps).
    ///
    /// Resuming is a round trip: a draft is opened from a list row, so the first open of one
    /// fetches it. The composer appears when the answer does. A draft that could not be opened is
    /// **said**, and the message opens for reading: a composer opened without the draft's content
    /// would replace it with what was on screen the next time it saved.
    private func resumeOrRead(
        _ account: String,
        _ key: String,
        otherwise read: @escaping () -> Void
    ) {
        guard model.showingDrafts else {
            read()
            return
        }
        beginCompose(.draft(account: account, key: key), failed: read)
    }

    /// Opens a conversation: expands the thread inline in the list (its messages become rows of
    /// their own) and opens its newest message in the reading pane, whatever its kind: a Sent
    /// reply filed elsewhere or a draft included (`docs/row-actions.md`, rules 2 and 8). In Drafts
    /// it is the newest draft (`latestKey`, the one the row summarises), which opens back into its
    /// composer as a flat row's does.
    ///
    /// The whole thing goes through the handover, expansion included, so a thread opened beside a
    /// composer replaces it like any other open.
    func open(_ thread: ThreadRow) {
        openGuardingDraft {
            expandedThreads.insert(threadKey(thread))
            let summarised = thread.messages.first(where: { $0.key == thread.latestKey })
            let representative = model.showingDrafts
                ? summarised ?? thread.messages.first : thread.messages.first
            if let representative {
                resumeOrRead(representative.account, representative.key) {
                    openThreadMessageNow(thread, representative)
                }
            }
        }
    }

    /// Opens one message of a conversation (a sub-row) in the reading pane, or back into its
    /// composer when the conversation is being shown in Drafts.
    func openThreadMessage(_ thread: ThreadRow, _ message: ThreadMessage) {
        openGuardingDraft {
            resumeOrRead(message.account, message.key) {
                openThreadMessageNow(thread, message)
            }
        }
    }

    // The unguarded open, so `open(_ thread:)`, already inside the guard, doesn't ask twice.
    private func openThreadMessageNow(_ thread: ThreadRow, _ message: ThreadMessage) {
        openNow(opened(message, subject: thread.subject))
    }

    /// Folds a conversation away, or opens it as `open(_:)` does: the chevron, and the row's
    /// primary action.
    func toggle(_ thread: ThreadRow) {
        if expandedThreads.contains(threadKey(thread)) {
            expandedThreads.remove(threadKey(thread))
        } else {
            open(thread)
        }
    }

    /// The expansion key for a thread (its account-scoped id).
    func threadKey(_ thread: ThreadRow) -> String { ConversationExpansion.key(thread) }

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

    /// One row of the list. None of them handles a click itself: the list's selection does, so a
    /// modifier, a drag across rows and ⌘A all mean what the platform says they mean
    /// (`docs/list-selection.md`, rule 1), and a row opens when it becomes the one selected.
    @ViewBuilder
    func entryView(_ entry: MessageListEntry) -> some View {
        switch entry {
        case .row(.flat(let message)): flatRow(message)
        case .row(.thread(let thread)): threadHeader(thread)
        case .member(let thread, let message): threadMessageRow(thread, message)
        }
    }

    private func flatRow(_ message: FlatRow) -> some View {
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
        // Onto a folder in the pane (`docs/folder-pane.md`, rule 24).
        .modifier(MessageDragSource(enabled: hasReadingPane) { dragPayload(for: .flat(row: message)) })
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

    /// A conversation row: subject, message count and latest sender. Expanded, every message on
    /// the thread follows it as a row of its own, received and the account owner's own Sent
    /// replies alike (the core gathers them across folders).
    private func threadHeader(_ thread: ThreadRow) -> some View {
        let expanded = expandedThreads.contains(threadKey(thread))
        // A conversation is unread if anything in it is: the header is a summary, and it may not
        // read as settled while it hides an unread reply. Its sub-rows carry their own weight.
        let unread = thread.unreadCount > 0
        return HStack(spacing: 8) {
            // The expand/collapse chevron sits in the same leading gutter a flat row reserves
            // (Outlook-style), so the conversation glyph and subject align with the flat rows.
            //
            // A button of its own, so folding a conversation away is not also selecting it.
            Button { toggle(thread) } label: {
                Image(systemName: expanded ? "chevron.down" : "chevron.right")
                    .font(.caption).foregroundStyle(.secondary).frame(width: 14)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(
                expanded ? L10n.a11y_collapse_conversation() : L10n.a11y_expand_conversation()
            )
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
        .contentShape(Rectangle())
        .modifier(MessageDragSource(enabled: hasReadingPane) { dragPayload(for: .thread(row: thread)) })
    }

    /// The "Sent" or "Draft" capsule beside a conversation message's sender.
    private func threadBadge(_ text: String) -> some View {
        Text(text)
            .font(.caption2)
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
            .background(.quaternary, in: Capsule())
    }

    /// One message of an expanded conversation: an indented row of its own (sender, Sent badge,
    /// preview, date), so it can be selected, opened and acted on as the message it is.
    private func threadMessageRow(_ thread: ThreadRow, _ message: ThreadMessage) -> some View {
        HStack(spacing: 8) {
            unreadDot(message.unread)
            AvatarView(avatar: message.avatar, diameter: 26)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(message.from)
                        .font(.subheadline)
                        .fontWeight(message.unread ? .semibold : .regular)
                        .lineLimit(1)
                    if message.outgoing {
                        threadBadge(L10n.thread_sent())
                    }
                    if message.draft {
                        threadBadge(L10n.thread_draft())
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
        .contentShape(Rectangle())
    }
}
