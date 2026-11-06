// Settings → Videoconferencing's list of the accounts that make video meetings (docs/settings.md
// category 4): each service through its account, in its state, opening that account's page, where
// its one switch is. No switch of its own here: two switches for one setting would be two places to
// change it, and two places to ask a provider for its permission.

import MailcalBindings
import SwiftUI

enum MeetingAccountLabels {
    /// The account's Video meetings use, as the row says it.
    static func state(_ state: CapabilityState) -> String {
        switch state {
        case .on: L10n.settings_toggle_on()
        case .off: L10n.settings_toggle_off()
        case .needsPermission: L10n.settings_meeting_account_needs_permission()
        case .notOffered: L10n.settings_meeting_account_not_offered()
        }
    }
}

struct MeetingAccountsList: View {
    var model: MailboxModel

    var body: some View {
        let rows = model.accountsSnapshot.meetingAccounts
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(rows.enumerated()), id: \.element.account.id) { index, row in
                if index > 0 { Divider() }
                // The open Settings follows `accountPage` to Accounts.
                Button { model.accountPage = row.account.id } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.service)
                            Text(AccountSettingsLabels.name(row.account))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(MeetingAccountLabels.state(row.state))
                            .foregroundStyle(.secondary)
                        Image(systemName: "chevron.right")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                            .accessibilityHidden(true)
                    }
                    .padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityHint(L10n.use_outage_open_account())
            }
        }
    }
}
