// What a setup sign-in button draws while its sign-in is out: the spinner, and the way back
// (docs/onboarding.md, "Nobody may be stranded on it"), drawn at once as the Nextcloud sign-in's
// is.

import MailcalBindings // L10n
import SwiftUI

struct SignInBusyRow: View {
    let escape: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.status_connecting()).foregroundStyle(.secondary)
            Button(L10n.action_cancel(), action: escape)
                .buttonStyle(.plain)
                .foregroundStyle(.tint)
        }
    }
}
