// The sign-in a separate server offers in the browser, at its own provider (docs/mail-oauth.md
// rule 13) or with Nextcloud (docs/nextcloud-sign-in.md), drawn above the login and password once
// the server has said it takes one: a large button, the line beneath it, and the password route
// behind "Use a password instead", as the IMAP card draws its own. The button runs as the core
// says, and while the browser holds the sign-in the line says it waits there, with its Cancel.

import MailcalBindings
import SwiftUI

struct SeparateServerSignInButton: View {
    var model: MailboxModel
    let form: SeparateServerFormModel
    let ask: SeparateServerAsk
    let way: AccountSeparateSignIn
    /// Whether Return presses the button while it leads: setup's step, not the account's page,
    /// whose Connect takes no Return either.
    var takesReturn = true

    var body: some View {
        VStack(spacing: 10) {
            button
            line
            if form.signIn.signInLeads(ask) {
                Button(L10n.setup_imap_signin_password_instead()) { form.choosePassword() }
                    .modifier(LinkButtonStyle())
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 4)
    }

    /// A press while the browser waits starts the sign-in again, as after a tab closed by
    /// accident.
    @ViewBuilder private var button: some View {
        let leads = form.signIn.signInLeads(ask)
        let label = ControlButton(
            title: SeparateServerLabels.signInButton(way), systemImage: "person.badge.key.fill",
            control: form.section.signInControl,
            shortcut: leads && takesReturn ? .defaultAction : nil, wide: true
        ) { form.startSignIn(model, ask) }
        .controlSize(.large)
        if leads {
            label.buttonStyle(.borderedProminent)
        } else {
            label.buttonStyle(.bordered)
        }
    }

    private var line: some View {
        HStack(spacing: 8) {
            Text(SeparateServerLabels.signInLine(way, form.section))
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let browser = SeparateServerRules.waiting(form.section) {
                ControlButton(title: L10n.action_cancel(), control: browser.cancel) {
                    form.cancelSignIn(model)
                }
            }
        }
    }
}
