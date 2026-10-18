// What the calendar screen says above the week about a calendar it cannot reach.

import MailcalBindings
import SwiftUI

extension CalendarScreenView {
    /// A Microsoft account connected before calendar support (or with revoked consent) syncs no
    /// calendar until it signs in again, prompt for it here, where its absence is felt. Mail is
    /// unaffected, so it uses an informational style, not the error one. "Reconnect" signs that
    /// account in again in place for everything it is used for (`docs/provider-oauth.md`
    /// rule 11); the banner clears once the calendar connects.
    @ViewBuilder var reauthBanner: some View {
        let ids = model.connectivity?.calendarReauthAccounts ?? []
        if let first = ids.first {
            // One button signs the first affected account in again; when several are affected the
            // banner re-renders after each clears, walking through them one at a time.
            ScreenBannerRow(
                text: L10n.calendar_reauth_prompt(accounts: ids.map(model.signInName).joined(separator: ", ")),
                action: L10n.calendar_reauth_action()
            ) { model.signInAgainFromBanner(first) }
        }
    }

    /// One line per account whose calendar is on and did not connect: what the week shows of it
    /// is what was last synced, and nothing on it can be changed or answered until it connects.
    var outageBanners: some View {
        UseOutageBanners(lines: model.calendarOutageLines) { model.openAccountPage($0) }
    }
}
