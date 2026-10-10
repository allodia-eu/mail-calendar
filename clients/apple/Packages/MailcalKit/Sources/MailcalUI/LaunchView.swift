// What the window shows while the core opens the mailbox at launch (docs/boot-sequence.md).

import MailcalBindings
import SwiftUI

/// A blank page, then, once the open has outlasted the core's threshold, a progress indicator and
/// what it is waiting for. A normal open ends inside the threshold, so most launches never see it.
struct LaunchView: View {
    @State private var showsStatus = false

    var body: some View {
        VStack(spacing: 12) {
            if showsStatus {
                ProgressView()
                Text(L10n.status_opening_mailbox())
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task {
            try? await Task.sleep(for: .milliseconds(Int(launchStatusAfterMs())))
            showsStatus = true
        }
    }
}
