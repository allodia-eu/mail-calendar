// "Sign in with Nextcloud", drawn where a server answered that it is Nextcloud
// (docs/nextcloud-sign-in.md rule 2): the line that explains it, then the button, and the password
// route behind "Use a password instead" where the surface has one. While the server is checked,
// and then while the browser holds the sign-in, the button gives way to what is happening and a
// Cancel.

import MailcalBindings // L10n
import SwiftUI

struct NextcloudSignInControl: View {
    var signIn: NextcloudSignIn
    /// Whether the sign-in is the surface's primary action; Return presses it while it is.
    var leads = true
    /// Offers the password route beneath, until it is chosen.
    var passwordInstead: (() -> Void)? = nil
    /// The line that explains the sign-in, above it; `nil` where the surface explains it itself.
    var note: String? = L10n.nextcloud_sign_in_note()
    /// Whether the button can be pressed: the surface has what the sign-in needs.
    var enabled = true
    let start: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let note {
                Text(note)
                    .font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if signIn.waiting {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(signIn.checking
                        ? L10n.setup_nextcloud_checking() : L10n.nextcloud_sign_in_waiting())
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
        .disabled(!enabled)
        if leads {
            label.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
        } else {
            label.buttonStyle(.bordered)
        }
    }
}
