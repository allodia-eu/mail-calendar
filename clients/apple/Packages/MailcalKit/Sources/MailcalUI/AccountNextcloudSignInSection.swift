// The Sign-in section of a calendar and contacts account, which the core gives one only at a
// Nextcloud server (docs/nextcloud-sign-in.md rules 2 and 7): "Sign in again" for an account a
// Nextcloud sign-in made, and the switch from a typed password for one whose server answered that
// it is Nextcloud. The sign-in itself is this client's to run.

import MailcalBindings
import SwiftUI

struct AccountNextcloudSignInSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let section: AccountSignIn

    private var signIn: NextcloudSignIn { model.accountNextcloud }
    /// The page's own sign-in: the model's holder is shared with the banner and other pages.
    private var ours: Bool { signIn.key == entry.id }

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_signin_heading(),
            description: AccountSettingsLabels.signInLine(section.line, entry)
        ) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(section.routes.enumerated()), id: \.offset) { _, route in
                    controls(route)
                }
            }
        }
        .onDisappear {
            if ours { signIn.leave(model.app) }
        }
    }

    @ViewBuilder
    private func controls(_ route: AccountSignInRoute) -> some View {
        if ours && signIn.waiting {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.nextcloud_sign_in_waiting())
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Button(L10n.action_cancel()) { signIn.cancel() }
            }
        } else if ours && signIn.completing {
            progress(L10n.status_connecting())
        } else {
            ControlButton(
                title: AccountSettingsLabels.signInRoute(route),
                control: section.control.held(signIn.completing)
            ) {
                model.signInAccountWithNextcloud(entry.id)
            }
        }
        if ours, let failure = signIn.failure {
            Text(failure).font(.callout).foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        } else if ours, signIn.succeeded == entry.id {
            Text(AccountSettingsLabels.signInDone(route))
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
