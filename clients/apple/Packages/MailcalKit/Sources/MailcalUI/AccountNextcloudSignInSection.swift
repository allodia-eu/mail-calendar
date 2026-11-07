// The Sign-in section of a calendar and contacts account, which has one only at a Nextcloud
// server (docs/nextcloud-sign-in.md rules 2 and 7): "Sign in again" for an account a Nextcloud
// sign-in made, and the switch from a typed password for one whose server answered that it is
// Nextcloud. It never switches to a typed password: the login is the one the server issued the
// password under.

import MailcalBindings
import SwiftUI

enum NextcloudAccountRules {
    struct Section: Equatable {
        let status: String
        /// `nil` until the server has answered, or when it answered no.
        let action: SignInAction?
    }

    /// The section for a calendar and contacts account, `nextcloud` when a Nextcloud sign-in made
    /// it (`AccountEntry.nextcloud`, read before its sign-in method, which reports a provider).
    /// `nil` when it has none.
    static func section(nextcloud: Bool, _ options: AccountSignInOptions?) -> Section? {
        let offered = options?.nextcloud == true
        guard nextcloud || offered else { return nil }
        let action: SignInAction? =
            offered ? (nextcloud ? .signInAgain : .toProvider(label: "Nextcloud")) : nil
        return Section(
            status: nextcloud
                ? L10n.settings_account_signs_in_nextcloud()
                : L10n.settings_account_signs_in_password(),
            action: action)
    }
}

struct AccountNextcloudSignInSection: View {
    var model: MailboxModel
    let entry: AccountEntry

    /// The server's answer, once the core has given it.
    @State private var options: AccountSignInOptions?

    private var signIn: NextcloudSignIn { model.accountNextcloud }
    /// The page's own sign-in: the model's holder is shared with the banner and other pages.
    private var ours: Bool { signIn.key == entry.id }

    var body: some View {
        Group {
            if let section = NextcloudAccountRules.section(nextcloud: entry.nextcloud, options) {
                SettingsGroup(
                    heading: L10n.settings_account_signin_heading(), description: section.status
                ) {
                    VStack(alignment: .leading, spacing: 8) {
                        if let action = section.action {
                            controls(action)
                        }
                    }
                }
            }
        }
        .task(id: entry.id) {
            options = nil
            options = await model.accountSignInOptions(entry.id)
        }
        .onDisappear {
            if ours { signIn.leave(model.app) }
        }
    }

    @ViewBuilder
    private func controls(_ action: SignInAction) -> some View {
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
            Button(AccountSettingsLabels.signInAction(action)) {
                model.signInAccountWithNextcloud(entry.id)
            }
            .disabled(signIn.completing)
        }
        if ours, let failure = signIn.failure {
            Text(failure).font(.callout).foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        } else if ours, signIn.succeeded == entry.id {
            Text(AccountSettingsLabels.signInDone(action))
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
