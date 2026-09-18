// The Outbox list: the messages that have not gone, and the three things a person can do
// about one.
//
// Its own list rather than a case inside the message list, because a queued message is not a
// stored one: it has no sender to show (it is the user), no read or flagged state, no date it
// arrived, and no provider key. What it has instead is a reason it is still here.

import MailcalBindings
import SwiftUI

extension ContentView {
    /// The queued sends, oldest first: the order they will go out in.
    var outboxList: some View {
        List {
            ForEach(model.outbox, id: \.op) { row in
                outboxRowView(row)
                    .contextMenu { outboxActions(row) }
            }
        }
        // An empty Outbox is not a state this list is ever left in: the row that opens it
        // disappears with the last message, and the core moves the list back to mail.
        .overlay {
            if model.outbox.isEmpty {
                ContentUnavailableView(
                    L10n.folder_outbox(),
                    systemImage: "tray.and.arrow.up",
                    description: Text(L10n.outbox_waiting())
                )
            }
        }
    }

    /// One unsent message: who it is for, what it says, and why it has not gone.
    @ViewBuilder func outboxRowView(_ row: QueuedRow) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(row.to.isEmpty ? row.account : row.to)
                .font(.body.weight(.semibold))
                .lineLimit(1)
            Text(row.subject)
                .font(.body)
                .lineLimit(1)
            HStack(spacing: 6) {
                Image(systemName: outboxStateIcon(row.state))
                    .font(.caption)
                Text(outboxStateLabel(row.state))
                    .font(.caption)
            }
            .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
        // Spoken as one row: three separate labels would be read as three items in a list of
        // messages, which is not what this is.
        .accessibilityElement(children: .combine)
    }

    /// The row's actions. A message already on its way, or one whose delivery could not be
    /// confirmed, offers **none**: neither can be called back, and offering to send an
    /// unconfirmed message again is how it arrives twice.
    @ViewBuilder func outboxActions(_ row: QueuedRow) -> some View {
        if model.queuedRowIsActionable(row) {
            Button(L10n.action_send_now()) { model.sendQueuedNow(row) }
            Button(L10n.action_edit_queued()) { model.editQueued(row) }
            Divider()
            Button(L10n.action_cancel_send(), role: .destructive) { model.cancelQueued(row) }
        }
    }

    /// What the row says about where the message has got to.
    func outboxStateLabel(_ state: QueuedState) -> String {
        switch state {
        case .waiting: L10n.outbox_waiting()
        case .sending: L10n.outbox_sending()
        case .unconfirmed: L10n.outbox_unconfirmed()
        }
    }

    /// The glyph beside it. Unconfirmed earns the warning: it is the one state a person may
    /// need to check on another device.
    func outboxStateIcon(_ state: QueuedState) -> String {
        switch state {
        case .waiting: "clock"
        case .sending: "arrow.up.circle"
        case .unconfirmed: "exclamationmark.triangle"
        }
    }
}
