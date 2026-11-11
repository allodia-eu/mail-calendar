// Settings → Videoconferencing's default per account (docs/conferencing.md, rule 10): the video
// meeting a new event in one of the account's calendars takes once it has an invitee. The core
// lists the accounts, what each offers and what is chosen; this draws a picker per account.

import MailcalBindings
import SwiftUI

struct DefaultMeetingList: View {
    var model: MailboxModel
    /// The accounts and their defaults, as the page read them from the core.
    let rows: [MeetingDefault]

    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(rows, id: \.account) { row in
                HStack(spacing: 8) {
                    Text(row.accountAddress)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: 0)
                    picker(row)
                }
            }
        }
    }

    /// One line per account, as a settings row is. A choice made through an account names that
    /// account, which is wider than a phone leaves beside the address, so on a phone the closed
    /// picker shows the service alone and the open menu the full label: two accounts offering
    /// one service stay two entries a person can tell apart.
    private var compact: Bool {
        #if os(iOS)
        sizeClass == .compact
        #else
        false
        #endif
    }

    @ViewBuilder
    private func picker(_ row: MeetingDefault) -> some View {
        if compact {
            Menu {
                choices(row).pickerStyle(.inline)
            } label: {
                HStack(spacing: 4) {
                    Text(row.choices.first { $0.id == row.chosen }?.name ?? L10n.event_meeting_none())
                    Image(systemName: "chevron.up.chevron.down")
                        .imageScale(.small)
                }
            }
            .fixedSize()
            .accessibilityLabel(row.accountAddress)
            .accessibilityValue(
                row.choices.first { $0.id == row.chosen }.map(EventEditorState.label)
                    ?? L10n.event_meeting_none()
            )
            .accessibilityIdentifier("settings-default-meeting-\(row.account)")
        } else {
            choices(row)
                .pickerStyle(.menu)
                .fixedSize()
                .accessibilityIdentifier("settings-default-meeting-\(row.account)")
        }
    }

    private func choices(_ row: MeetingDefault) -> some View {
        Picker(
            row.accountAddress,
            selection: Binding(
                get: { row.chosen },
                set: { model.setDefaultMeeting(account: row.account, choice: $0) }
            )
        ) {
            Text(L10n.event_meeting_none()).tag(String?.none)
            ForEach(row.choices, id: \.id) { choice in
                Text(EventEditorState.label(choice)).tag(Optional(choice.id))
            }
        }
        .labelsHidden()
    }
}
