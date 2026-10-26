// The sign-in at a separate server's own provider (docs/mail-oauth.md rule 13), drawn above the
// login and password once the server has said it takes one: a large button, the note beneath it,
// and the password route behind "Use a password instead", as the IMAP card draws its own. While
// the browser is out the button gives way to the busy row and, past its threshold, the way back.

import MailcalBindings // L10n
import SwiftUI

struct SeparateServerSignInButton: View {
    @Bindable var form: SeparateServerFormModel
    let ask: SeparateServerAsk
    /// Whether Return presses the button while it leads: setup's step, not the account's page,
    /// whose Connect takes no Return either.
    var takesReturn = true
    let start: () -> Void

    var body: some View {
        VStack(spacing: 10) {
            if form.signIn.signingIn {
                SignInBusyRow(escapable: form.signIn.escapable) { form.retire() }
                    .frame(minHeight: 32)
            } else {
                button
            }
            Text(L10n.setup_imap_signin_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            if form.signIn.signInLeads(ask) {
                Button(L10n.setup_imap_signin_password_instead()) { form.choosePassword() }
                    .modifier(LinkButtonStyle())
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 4)
        .signInEscapeClock($form.signIn.attempts)
    }

    @ViewBuilder private var button: some View {
        let label = Button(action: start) {
            Label(L10n.setup_imap_signin_button(), systemImage: "person.badge.key.fill")
                .frame(maxWidth: .infinity)
        }
        .controlSize(.large)
        .disabled(form.holding)
        if form.signIn.signInLeads(ask) {
            if takesReturn {
                label.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
            } else {
                label.buttonStyle(.borderedProminent)
            }
        } else {
            label.buttonStyle(.bordered)
        }
    }
}
