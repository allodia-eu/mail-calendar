// What a setup sign-in button draws while its browser is out: the spinner, and once the hop has
// outlasted `SignInAttempts.escapeAfter`, the way back (docs/onboarding.md, "Nobody may be
// stranded on it").

import MailcalBindings // L10n
import SwiftUI

struct SignInBusyRow: View {
    let escapable: Bool
    let escape: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.status_connecting()).foregroundStyle(.secondary)
            if escapable {
                Button(L10n.action_cancel(), action: escape)
                    .buttonStyle(.plain)
                    .foregroundStyle(.tint)
            }
        }
    }
}

extension View {
    /// Arms the way back for the attempt in flight. Keyed on that attempt, so SwiftUI restarts the
    /// clock when one begins and cancels it when one ends: an older attempt's clock never arms a
    /// newer one, and no clock outlives the button.
    func signInEscapeClock(_ attempts: Binding<SignInAttempts>) -> some View {
        task(id: attempts.wrappedValue.current) {
            guard let attempt = attempts.wrappedValue.current else { return }
            try? await Task.sleep(for: SignInAttempts.escapeAfter)
            // `try?` swallows the cancellation that ending the attempt raises.
            if !Task.isCancelled { attempts.wrappedValue.outlasted(attempt) }
        }
    }
}
