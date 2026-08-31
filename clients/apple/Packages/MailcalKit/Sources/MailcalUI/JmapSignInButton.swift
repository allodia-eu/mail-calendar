// The "Sign in with your provider" block of the JMAP setup form, shared by the manual form
// (AccountSetupView) and the detected-JMAP card (AccountSetupDetectView) so the two cannot drift.
//
// Two rules shape it, both from docs/jmap.md:
//   - It is an ADDITION, never a replacement. The password/API-token field sits right below and
//     always works; sign-in is discoverable, so any server may decline it.
//   - It is never a dead end. The button appears only after the core confirmed this server
//     advertises OAuth (`jmapOauthAvailable`), and a failed sign-in says so inline and leaves the
//     secret field usable.
//
// Availability is a blocking core call, so `jmapOAuthProbe` runs it off the main thread and
// debounces it: `.task(id:)` cancels the previous probe on every keystroke, and the sleep in front
// of the call is what keeps a network round trip off each character typed. The probe rides on a
// view that is always there (the email/secret field) and the button is mounted only once it says
// yes, inside a Form, a view that renders nothing still occupies a row, which left a blank gap
// between the email and secret fields.

import MailcalBindings // L10n
import SwiftUI

/// The debounced availability probe behind the sign-in button.
enum JmapOAuthProbe {
    /// How long the fields must sit still before a probe goes out.
    static let debounce = Duration.milliseconds(600)

    /// Whether `email` is complete enough to be worth a network probe. Deliberately crude, the
    /// core validates for real, but it keeps half-typed addresses from firing a request per
    /// keystroke at whatever domain they momentarily spell.
    static func looksLikeAddress(_ email: String) -> Bool {
        let parts = email.split(separator: "@", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty else { return false }
        let domain = parts[1]
        return domain.contains(".") && !domain.hasPrefix(".") && !domain.hasSuffix(".")
    }
}

extension View {
    /// Asks the core whether this JMAP server offers sign-in and mirrors the answer into `offered`,
    /// which gates whether `JmapSignInButton` is shown at all. Attach it to a view that is present
    /// for the whole JMAP branch; the answer is dropped (and the button hidden) the moment the
    /// address changes, so a button is never left over from a different server.
    func jmapOAuthProbe(
        email: String,
        serverURL: String,
        isAvailable: @escaping (String, String) async -> Bool,
        offered: Binding<Bool>
    ) -> some View {
        task(id: "\(email)|\(serverURL)") {
            offered.wrappedValue = false
            guard JmapOAuthProbe.looksLikeAddress(email) else { return }
            try? await Task.sleep(for: JmapOAuthProbe.debounce)
            guard !Task.isCancelled else { return }
            let result = await isAvailable(email, serverURL)
            // The user kept typing while the (blocking, uncancellable) probe ran, its answer is
            // about an address they have already moved on from.
            guard !Task.isCancelled else { return }
            offered.wrappedValue = result
        }
    }
}

/// The button itself, shown only when `jmapOAuthProbe` said this server offers sign-in.
struct JmapSignInButton: View {
    /// The address being connected.
    let email: String
    /// The typed/detected server URL; empty means "derive it from the email domain".
    let serverURL: String
    /// Runs the browser sign-in and, on success, adds + stores the account.
    let signIn: (String, String) async -> JmapSignInOutcome

    @State private var signingIn = false
    @State private var failed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L10n.setup_jmap_signin_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if signingIn {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.status_connecting()).foregroundStyle(.secondary)
                }
            } else {
                Button(L10n.setup_jmap_signin_button()) { start() }
            }
            // Kept visible after a failure even though the button is still there: this copy is what
            // tells the user the secret field below is the way through.
            if failed {
                Text(L10n.setup_jmap_signin_failed())
                    .font(.caption).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func start() {
        Task {
            failed = false
            signingIn = true
            let outcome = await signIn(email, serverURL)
            signingIn = false
            // A dismissed browser is not a failure, say nothing and leave the button as it was.
            if case .failed = outcome { failed = true }
        }
    }
}
