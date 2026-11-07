// Settings → Videoconferencing's list of the accounts that make video meetings (docs/settings.md
// category 4): what each row says.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct MeetingAccountsListTests {
    @Test func eachStateIsSaidAsTheAccountPageMeansIt() {
        #expect(MeetingAccountLabels.state(.on) == L10n.settings_toggle_on())
        #expect(MeetingAccountLabels.state(.off) == L10n.settings_toggle_off())
        #expect(
            MeetingAccountLabels.state(.needsPermission)
                == L10n.settings_meeting_account_needs_permission())
        #expect(MeetingAccountLabels.state(.notOffered) == L10n.settings_meeting_account_not_offered())
    }
}
