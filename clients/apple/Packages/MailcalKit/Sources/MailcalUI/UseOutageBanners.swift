// The banner row the calendar and contacts screens draw above what they show: a warning, one
// sentence or three, and one action. A material and a hairline below, so the rows scroll on
// underneath it.

import MailcalBindings
import SwiftUI

struct ScreenBannerRow: View {
    let text: String
    let action: String
    let onAction: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
            Text(text).font(.callout).fixedSize(horizontal: false, vertical: true)
            Spacer()
            Button(action, action: onAction).buttonStyle(.borderless)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .background(.thinMaterial)
        .overlay(alignment: .bottom) { Divider() }
    }
}

/// One row per calendar or address book that did not connect, in the snapshot's order. The action
/// opens that account's page in Settings → Accounts, where the same reason sits under the use's
/// switch, or asks about a server on another site where that is what the use waits on.
struct UseOutageBanners: View {
    var model: MailboxModel
    let lines: [UseOutageLine]

    @State private var serverChange: ServerChangeReview?

    var body: some View {
        VStack(spacing: 0) {
            ForEach(lines) { line in
                if line.reviews {
                    ScreenBannerRow(text: line.text, action: L10n.server_changed_review()) {
                        serverChange = model.serverChangeReview(line.account)
                    }
                } else {
                    ScreenBannerRow(text: line.text, action: L10n.use_outage_open_account()) {
                        model.openAccountPage(line.account)
                    }
                }
            }
        }
        .serverChangeReview(model, $serverChange)
    }
}
