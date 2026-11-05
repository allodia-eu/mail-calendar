// "Sign in with Nextcloud", drawn where a server answered that it is Nextcloud
// (docs/nextcloud-sign-in.md rule 2): the line that explains it, then the button, and the password
// route behind "Use a password instead" where the surface has one. While the browser holds the
// sign-in the button gives way to what to do there and a Cancel.

import MailcalBindings // L10n
import SwiftUI

struct NextcloudSignInControl: View {
    var signIn: NextcloudSignIn
    /// Whether the sign-in is the surface's primary action.
    var leads = true
    /// Whether Return presses it while it leads.
    var takesReturn = true
    /// Offers the password route beneath, until it is chosen.
    var passwordInstead: (() -> Void)? = nil
    let start: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L10n.nextcloud_sign_in_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if signIn.waiting {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.nextcloud_sign_in_waiting())
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                    Button(L10n.action_cancel()) { signIn.cancel() }
                }
                .frame(minHeight: 32)
            } else if signIn.completing {
                progress(L10n.status_connecting()).frame(minHeight: 32)
            } else {
                button
            }
            if let failure = signIn.failure {
                Text(failure).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let passwordInstead {
                Button(L10n.setup_imap_signin_password_instead(), action: passwordInstead)
                    .modifier(LinkButtonStyle())
                    .disabled(signIn.busy)
                    .frame(maxWidth: .infinity)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder private var button: some View {
        let label = Button(action: start) {
            Label(L10n.nextcloud_sign_in_button(), systemImage: "person.badge.key.fill")
                .frame(maxWidth: .infinity)
        }
        .controlSize(.large)
        if leads && takesReturn {
            label.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
        } else if leads {
            label.buttonStyle(.borderedProminent)
        } else {
            label.buttonStyle(.bordered)
        }
    }
}
