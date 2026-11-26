// The manual form's Nextcloud pane: the server's address and its own sign-in, for a person whose
// email address leads detection nowhere near their Nextcloud server
// (docs/nextcloud-sign-in.md rule 2). The core asks the address whether it is Nextcloud before a
// browser opens. When it cannot tell, the pane says so and offers the sign-in anyway, for as long
// as the field holds the address the core was asked about.

import MailcalBindings
import SwiftUI

extension AccountSetupView {
    var nextcloudForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_nextcloud_note())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_account_type_nextcloud(), systemImage: "cloud") {
                TextField(L10n.setup_field_nextcloud_server(), text: $nextcloudServer)
                    .setupField(.host)
                    .disabled(model.setupNextcloud.busy)
                    .onTouchKeyboardSubmit {
                        guard canSignInWithNextcloud, !model.setupNextcloud.busy else { return }
                        model.signInWithNextcloud(at: typedNextcloudServer, anyway: false)
                    }
                if let host = model.setupNextcloud.unsure?.offer(nextcloudServer) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(L10n.setup_nextcloud_unsure(host: host))
                            .font(.callout)
                            .fixedSize(horizontal: false, vertical: true)
                        Button(L10n.setup_nextcloud_try_anyway()) {
                            model.signInWithNextcloud(at: typedNextcloudServer, anyway: true)
                        }
                        .buttonStyle(.bordered)
                    }
                }
                NextcloudSignInControl(
                    signIn: model.setupNextcloud, note: nil, enabled: canSignInWithNextcloud
                ) {
                    model.signInWithNextcloud(at: typedNextcloudServer, anyway: false)
                }
                .disabled(model.isConnecting || model.updatingExistingAccount)
            }
        }
    }

    var typedNextcloudServer: String {
        nextcloudServer.trimmingCharacters(in: .whitespaces)
    }

    var canSignInWithNextcloud: Bool { !typedNextcloudServer.isEmpty }
}
