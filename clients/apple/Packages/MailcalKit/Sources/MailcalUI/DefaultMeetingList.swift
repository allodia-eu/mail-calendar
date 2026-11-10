// Settings → Videoconferencing's default per account (docs/conferencing.md, rule 10): the video
// meeting a new event in one of the account's calendars takes once it has an invitee. The core
// lists the accounts, what each offers and what is chosen; this draws a picker per account.

import MailcalBindings
import SwiftUI

struct DefaultMeetingList: View {
    var model: MailboxModel

    var body: some View {
        // The core signals the calendar surface when a default changes.
        let _ = model.calendarVersion
        VStack(alignment: .leading, spacing: 8) {
            ForEach(model.meetingDefaults(), id: \.account) { row in
                HStack(spacing: 8) {
                    Text(row.accountAddress)
                    Spacer(minLength: 0)
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
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .fixedSize()
                    .accessibilityIdentifier("settings-default-meeting-\(row.account)")
                }
            }
        }
    }
}
