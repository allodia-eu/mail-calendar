// Why a mail list has no rows, said on the list itself.
//
// The badge beside the folder counts what the server holds, over all time, while the list can
// only show what sync depth kept (`docs/folder-pane.md`, rule 5). A folder of older mail then
// badges its unread above no rows at all, and without this nothing on screen reconciles the two
// numbers. Which of the two cases it is comes from the core, so this view chooses no wording of
// its own beyond the copy.

import MailcalBindings
import SwiftUI

/// The empty-list explanation, or nothing at all when the list has rows.
///
/// One view for both layouts, as `SearchHorizonStrip` is: the phone and the desktop put it in
/// different places, but what it says, and when it says nothing, is one decision.
struct EmptyMailboxView: View {
    let reason: EmptyReason?
    let openSettings: () -> Void

    var body: some View {
        switch reason {
        case .none:
            EmptyView()
        case .noMail:
            // Nothing was held back, so there is no setting to offer: an action here would
            // promise mail that widening cannot find.
            ContentUnavailableView(
                L10n.mailbox_empty_no_mail(),
                systemImage: "tray"
            )
        case .outsideSyncDepth(let months):
            ContentUnavailableView {
                Label(L10n.mailbox_empty_windowed(count: Int(months)), systemImage: "clock.arrow.circlepath")
            } description: {
                Text(L10n.mailbox_empty_windowed_body())
            } actions: {
                Button(L10n.mailbox_empty_change(), action: openSettings)
            }
        }
    }
}
