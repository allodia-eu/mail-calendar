// The Gmail Early Access gate, shared by both Google sign-in entry points (the manual account
// picker in AccountSetupView and the detection found-card in AccountSetupDetectView). While
// Google verifies the app, only allow-listed OAuth test users can complete the flow, so we make
// the user confirm they've signed up before enabling "Sign in with Google", otherwise Google
// hard-blocks the flow with a confusing error. One definition so the confirmation stays a single
// contract; each host owns the `confirmed` state and disables its sign-in button until it is on.

import MailcalBindings
import SwiftUI

/// A short Early Access explainer, a link to sign up (opens in the system browser), and the
/// mandatory confirmation toggle. `confirmed` is the caller's gate for enabling sign-in.
struct GoogleEarlyAccessGate: View {
    @Binding var confirmed: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L10n.setup_google_early_access_title()).font(.headline)
            Text(L10n.setup_google_early_access_body())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if let url = URL(string: L10n.setup_google_early_access_url()) {
                Link(L10n.setup_google_early_access_link(), destination: url)
                    .font(.callout)
            }
            Toggle(L10n.setup_google_early_access_confirm(), isOn: $confirmed)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
