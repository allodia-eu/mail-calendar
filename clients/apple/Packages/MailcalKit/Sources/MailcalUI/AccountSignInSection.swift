// How a standards or JMAP account signs in, on its page: the way it signs in now, signing in
// again when that is through its provider, and the switch to the other way in where the server
// takes it (docs/accounts.md rule 17). The switch is drawn once the core has asked the server,
// which can take a while; until then the section says only how the account signs in.

import MailcalBindings
import SwiftUI

struct AccountSignInSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    @Binding var alert: AccountPageAlert?

    /// What the server takes, once the core has answered for the way the account signs in now.
    @State private var options: AccountSignInOptions?
    /// The line the section ends with once a control has worked.
    @State private var result: String?
    /// The password field is drawn: the person chose the switch to a password.
    @State private var choosingPassword = false
    @State private var password = ""
    /// The question a switch asks before the browser opens, while it is on screen.
    @State private var question: UnreachedQuestion?

    /// The core is working on this account's sign-in, so nothing here may start another.
    private var busy: Bool { model.signInSwitchBusy == entry.id }
    /// The browser is up for this account. Its own button stays live, because a sign-in
    /// abandoned in the browser cannot be told from one in progress (docs/provider-oauth.md
    /// rule 13).
    private var waiting: Bool { model.signingInAgain == entry.id }

    var body: some View {
        let actions = AccountSignInRules.actions(entry.signIn, options)
        SettingsGroup(
            heading: L10n.settings_account_signin_heading(),
            description: AccountSettingsLabels.signInMethod(entry.signIn)
        ) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(actions.filter(\.opensBrowser), id: \.self) { action in
                    Button(AccountSettingsLabels.signInAction(action)) { throughProvider(action) }
                        .disabled(busy)
                }
                if actions.contains(.toPassword) {
                    passwordSwitch
                }
                status
                if AccountSignInRules.offersSwitch(actions) {
                    Text(L10n.settings_account_switch_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .unreachedUsesAlert($question)
        // Asked again once a switch lands, because the other way in is a different question.
        .task(id: OptionsKey(account: entry.id, signIn: entry.signIn)) {
            options = nil
            options = await model.accountSignInOptions(entry.id)
        }
    }

    /// The switch to a password: a button, then the field it opens. The field never shows the
    /// stored credential.
    @ViewBuilder
    private var passwordSwitch: some View {
        if choosingPassword {
            SecureField(L10n.setup_field_password(), text: $password)
                .setupField(.password)
                .frame(maxWidth: 320)
                .disabled(busy)
            HStack(spacing: 10) {
                Button(L10n.settings_account_switch_password_save(), action: switchToPassword)
                    .buttonStyle(.borderedProminent)
                    .disabled(password.isEmpty || busy || waiting)
                Button(L10n.action_cancel()) {
                    choosingPassword = false
                    password = ""
                }
                .disabled(busy)
            }
        } else {
            Button(AccountSettingsLabels.signInAction(.toPassword)) {
                result = nil
                choosingPassword = true
            }
            .disabled(busy || waiting)
        }
    }

    @ViewBuilder
    private var status: some View {
        if waiting {
            progress(L10n.settings_account_signin_waiting(email: AccountSettingsLabels.name(entry)))
        } else if busy {
            progress(L10n.status_connecting())
        } else if let result {
            Text(result)
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// Signs in through the provider in the browser, and says how it ended. A cancelled one says
    /// nothing: the person closed it, or pressed again.
    private func throughProvider(_ action: SignInAction) {
        guard let redirect = AccountSignInRules.redirectURI(entry.kind) else { return }
        result = nil
        let id = entry.id
        Task {
            let ended = await model.signInThroughProvider(id, redirectURI: redirect) { set in
                await UnreachedQuestion.ask(set) { question = $0 }
            }
            switch ended {
            case .signedIn:
                result = AccountSettingsLabels.signInDone(action)
            case .cancelled:
                break
            case let .failed(detail, _):
                alert = AccountPageAlert(
                    title: AccountSettingsLabels.signInFailed(action), detail: detail)
            }
        }
    }

    private func switchToPassword() {
        let (id, typed) = (entry.id, password)
        result = nil
        Task {
            if let failure = await model.switchAccountToPassword(id, typed) {
                alert = AccountPageAlert(
                    title: AccountSettingsLabels.signInFailed(.toPassword), detail: failure)
            } else {
                password = ""
                choosingPassword = false
                result = AccountSettingsLabels.signInDone(.toPassword)
            }
        }
    }
}

/// What the section asks the core about: the account, and the way it signs in now.
private struct OptionsKey: Equatable {
    let account: String
    let signIn: SignInMethod
}
