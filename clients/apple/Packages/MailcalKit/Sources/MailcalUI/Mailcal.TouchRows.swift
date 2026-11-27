// The message rows of the touch layouts (the iPhone list and the iPad's list column), in the order
// Apple's own Mail uses: sender and date, then the subject, then two lines of the message. The Mac
// keeps the desktop row in Mailcal.MailRows.swift, which also holds what both share.

#if os(iOS)
import SwiftUI
import MailcalBindings

extension ContentView {
    func touchFlatRow(_ message: FlatRow) -> some View {
        TouchMailRow(
            unread: message.unread,
            avatar: message.avatar,
            sender: message.from,
            date: touchDate(message.date),
            subject: message.subject,
            importance: message.importance,
            flagged: message.flagged,
            hasAttachment: message.hasAttachment,
            preview: message.preview
        ) { EmptyView() }
    }

    /// A conversation: its latest sender and date, its subject with the message count, its newest
    /// message's text, and the chevron that folds it open or away after the date.
    ///
    /// The chevron is a button of its own, so folding a conversation is not also selecting it.
    /// It draws small but answers a 44-point square, the touch target the platform asks for,
    /// without making the line it sits on any taller.
    func touchThreadHeader(_ thread: ThreadRow, expanded: Bool, unread: Bool) -> some View {
        TouchMailRow(
            unread: unread,
            avatar: thread.avatar,
            sender: thread.latestFrom,
            date: touchDate(thread.latestDate),
            subject: thread.subject,
            count: thread.messageCount,
            importance: thread.importance,
            flagged: thread.flagged,
            hasAttachment: thread.hasAttachment,
            preview: thread.preview
        ) {
            Button { toggle(thread) } label: {
                Image(systemName: expanded ? "chevron.down" : "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.tint)
                    .frame(width: 16, height: 16)
                    .padding(14)
                    .contentShape(Rectangle())
                    .padding(-14)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(
                expanded ? L10n.a11y_collapse_conversation() : L10n.a11y_expand_conversation()
            )
        }
    }

    /// One message of an open conversation: the subject is the conversation's, so the row starts
    /// with who wrote it, set in under the conversation's own avatar.
    func touchThreadMessageRow(_ message: ThreadMessage) -> some View {
        TouchMailRow(
            unread: message.unread,
            avatar: message.avatar,
            avatarDiameter: 30,
            indent: 22,
            sender: message.from,
            badges: (message.outgoing ? [L10n.thread_sent()] : [])
                + (message.draft ? [L10n.thread_draft()] : []),
            date: touchDate(message.date),
            subject: nil,
            importance: message.importance,
            flagged: message.flagged,
            hasAttachment: message.hasAttachment,
            preview: message.preview
        ) { EmptyView() }
    }

    private func touchDate(_ date: String) -> String {
        relativeDate(date, in: model.activeZone, use24Hour: model.use24Hour)
    }
}

/// One touch row. The unread dot stands in the list's leading margin, as in Apple's Mail, so it
/// costs the text no width; a read row keeps the dot's space, and every avatar and subject stays
/// in one column down the list.
private struct TouchMailRow<Trailing: View>: View {
    let unread: Bool
    let avatar: Avatar
    var avatarDiameter: CGFloat = 40
    var indent: CGFloat = 0
    let sender: String
    var badges: [String] = []
    let date: String
    /// `nil` for a row with no subject line of its own; empty for a message without a subject.
    let subject: String?
    var count: UInt32 = 0
    let importance: MessageImportance
    let flagged: Bool
    let hasAttachment: Bool
    let preview: String
    @ViewBuilder let trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            UnreadDot(unread: unread, diameter: 10)
                .frame(width: 18, height: avatarDiameter)
            AvatarView(avatar: avatar, diameter: avatarDiameter)
                .padding(.leading, indent)
            VStack(alignment: .leading, spacing: 2) {
                senderLine
                if let subject {
                    subjectLine(subject)
                } else {
                    marks
                }
                if !preview.isEmpty {
                    Text(preview)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }
            .padding(.leading, 4)
        }
        .listRowInsets(EdgeInsets(top: 10, leading: 8, bottom: 10, trailing: 16))
    }

    /// Bold for unread on the sender and the subject, and nowhere else (`docs/capabilities.md`).
    private var senderLine: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(sender)
                .fontWeight(unread ? .semibold : .regular)
                .lineLimit(1)
            ForEach(badges, id: \.self) { badge in
                Text(badge)
                    .font(.caption2)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 1)
                    .background(.quaternary, in: Capsule())
            }
            Spacer(minLength: 4)
            Text(date)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .layoutPriority(1)
            trailing()
        }
    }

    private func subjectLine(_ subject: String) -> some View {
        HStack(spacing: 6) {
            Text(subject.isEmpty ? L10n.mail_no_subject() : subject)
                .font(.subheadline)
                .fontWeight(unread ? .semibold : .regular)
                .lineLimit(1)
            if count > 1 {
                Text("\(count)")
                    .font(.caption2)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 1)
                    .background(.quaternary, in: Capsule())
            }
            Spacer(minLength: 0)
            marks
        }
    }

    /// Importance, flag and attachment, at the end of the subject line, under the date.
    @ViewBuilder
    private var marks: some View {
        if importance != .normal || flagged || hasAttachment {
            HStack(spacing: 6) {
                if subject == nil { Spacer(minLength: 0) }
                ImportanceMark(importance: importance)
                if flagged {
                    Image(systemName: "flag.fill").foregroundStyle(.orange)
                }
                if hasAttachment {
                    Image(systemName: "paperclip").foregroundStyle(.secondary)
                }
            }
            .font(.footnote)
        }
    }
}
#endif
