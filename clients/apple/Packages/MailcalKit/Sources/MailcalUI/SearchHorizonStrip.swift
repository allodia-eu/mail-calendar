// How far back a search looked, said on the results surface.
//
// Search reads what is on this device and nothing else, so it finds only what sync depth kept
// (`docs/search.md`). Without this line an empty result claims "no such message" when what it
// means is "not in the last three months", and the second is something the user can fix, which
// is why the strip carries a route to the setting rather than only the fact.

import MailcalBindings
import SwiftUI

/// The horizon line, or nothing at all when the list is not a search.
///
/// One view for both layouts: the desktop draws it under the list header and the phone under the
/// search field, but what it says, and when it says nothing, is one decision.
struct SearchHorizonStrip: View {
    let horizon: SearchHorizon?
    let openSettings: () -> Void

    var body: some View {
        if let horizon {
            HStack(spacing: 8) {
                Text(Self.label(horizon)).font(.caption).foregroundStyle(.secondary)
                Button(L10n.search_horizon_change(), action: openSettings)
                    .buttonStyle(.borderless)
                    .font(.caption)
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
        }
    }

    /// The horizon as the user reads it. The month count is the core's, so this line and the
    /// sync-depth setting cannot disagree about what the device holds.
    static func label(_ horizon: SearchHorizon) -> String {
        switch horizon {
        case .allTime: L10n.search_horizon_all()
        case .months(let months): L10n.search_horizon_months(count: Int(months))
        }
    }
}
