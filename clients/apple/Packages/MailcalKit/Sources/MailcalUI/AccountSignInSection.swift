// How an account signs in, on its page, drawn from the core's record (docs/accounts.md rule 17):
// the line saying how it signs in or what is wrong, the ways in through a browser, the browser
// sign-in while one is out, and the field a password is typed into. Which of them it offers, and
// every step of a browser sign-in, are the core's; the browser itself is MailcalModel's to open.

import MailcalBindings
import SwiftUI

struct AccountSignInSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let section: AccountSignIn

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_signin_heading(),
            description: AccountSettingsLabels.signInLine(section.line, entry)
        ) {
            VStack(alignment: .leading, spacing: 8) {
                // A route pressed while the browser waits starts the sign-in again
                // (docs/provider-oauth.md rule 13).
                ForEach(Array(section.routes.enumerated()), id: \.offset) { _, route in
                    ControlButton(
                        title: AccountSettingsLabels.signInRoute(route), control: section.control
                    ) { model.signInThroughBrowser(entry.id) }
                }
                if let browser = section.browser,
                   let waiting = AccountSettingsLabels.signInWaiting(
                       browser, name: AccountSettingsLabels.name(entry))
                {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text(waiting)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 0)
                        ControlButton(title: L10n.action_cancel(), control: browser.cancel) {
                            model.cancelSignIn(browser)
                        }
                    }
                }
                if let field = section.password {
                    AccountPasswordEntry(model: model, accountId: entry.id, field: field)
                }
                if section.switches {
                    Text(L10n.settings_account_switch_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .signInQuestion(AccountSignInRules.question(section.browser)) { attempt, answer in
            model.answerSignInQuestion(entry.id, attempt: attempt, answer)
        }
    }
}

/// The password field a Sign-in section offers. One that switches from the provider's sign-in
/// waits behind "Use a password instead" until it runs; one that replaces a refused password is
/// drawn at once. It never shows the stored credential, and empties as it is sent.
struct AccountPasswordEntry: View {
    var model: MailboxModel
    let accountId: String
    let field: AccountPasswordField

    @State private var revealed = false
    @State private var password = ""

    var body: some View {
        if field.replaces || revealed || field.save.running {
            HeldFields(control: field.save) {
                SecureField(title, text: $password)
                    .setupField(.password)
                    .frame(maxWidth: 320)
            }
            ControlButton(
                title: field.replaces
                    ? L10n.action_save() : L10n.settings_account_switch_password_save(),
                control: field.save,
                ready: !password.isEmpty
            ) {
                model.changeAccount(
                    .usePassword(account: accountId, password: TypedPassword(text: password)))
                password = ""
            }
            .buttonStyle(.borderedProminent)
        } else {
            // It opens the field and starts nothing.
            ControlButton(
                title: L10n.settings_account_switch_to_password(),
                control: .free(field.save.enabled)
            ) { revealed = true }
        }
    }

    private var title: String {
        switch field.kind {
        case .password: L10n.setup_field_password()
        case .passwordOrToken: L10n.setup_jmap_secret_placeholder()
        }
    }
}
