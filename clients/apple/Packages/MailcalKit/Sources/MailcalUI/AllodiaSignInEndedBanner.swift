// The banner a refused Allodia sign-in raises above the mailbox, beside the mail accounts' own
// expired sign-in banner. The rule is `entitlement.md`'s: the core decides when it shows
// (`ConnectivitySnapshot.allodiaSignInEnded`) and takes it down after a sign-in or a sign-out.

import MailcalBindings
import SwiftUI

/// Whose Allodia sign-in ended, and signing in again from where the person is.
struct AllodiaSignInEndedBanner: View {
    var model: MailboxModel
    let email: String

    /// A sign-in from this banner is in flight, so a second press does not start another.
    @State private var signingIn = false

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
            Text(L10n.allodia_signin_ended_prompt(email: email))
                .font(.callout).fixedSize(horizontal: false, vertical: true)
            Spacer()
            Button(L10n.signin_expired_action()) { signIn() }
                .buttonStyle(.borderless)
                .disabled(signingIn)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .bannerBackground()
        .overlay(alignment: .bottom) { Divider() }
    }

    private func signIn() {
        Task {
            signingIn = true
            let outcome = await model.signInToAllodia()
            signingIn = false
            switch outcome {
            // The core takes the banner down itself; what follows is what any sign-in starts.
            case .signedIn: await model.syncAllodiaAccounts()
            // A dismissed browser is not a failure and says nothing.
            case .cancelled: break
            case .failed: model.accountNotice = L10n.signin_expired_failed()
            }
        }
    }
}
