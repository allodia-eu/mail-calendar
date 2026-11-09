// How a standards or JMAP account signs in, on its page, drawn from the core's record
// (docs/accounts.md rule 17): the line saying how it signs in or what is wrong, the ways in
// through a browser, and the field a password is typed into. Which of them it offers is the
// core's; the browser sign-in is still this client's to run.

import MailcalBindings
import SwiftUI

struct AccountSignInSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let section: AccountSignIn
    @Binding var alert: AccountPageAlert?

    /// The line the section ends with once a browser sign-in has worked.
    @State private var result: String?
    /// The question a switch asks before the browser opens, while it is on screen.
    @State private var question: UnreachedQuestion?

    /// The core is starting or completing this account's browser sign-in.
    private var busy: Bool { model.signInSwitchBusy == entry.id }
    /// The browser is up for this account. Its own button stays live, because a sign-in
    /// abandoned in the browser cannot be told from one in progress (docs/provider-oauth.md
    /// rule 13).
    private var waiting: Bool { model.signingInAgain == entry.id }

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_signin_heading(),
            description: AccountSettingsLabels.signInLine(section.line, entry)
        ) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(section.routes.enumerated()), id: \.offset) { _, route in
                    ControlButton(
                        title: AccountSettingsLabels.signInRoute(route),
                        control: section.control.held(busy)
                    ) { throughProvider(route) }
                }
                if let field = section.password {
                    AccountPasswordEntry(
                        model: model, accountId: entry.id, field: field, held: busy || waiting)
                }
                status
                if section.switches {
                    Text(L10n.settings_account_switch_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .unreachedUsesAlert($question)
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
    private func throughProvider(_ route: AccountSignInRoute) {
        guard let redirect = AccountSignInRules.redirectURI(entry.kind) else { return }
        result = nil
        let id = entry.id
        Task {
            let ended = await model.signInThroughProvider(id, redirectURI: redirect) { set in
                await UnreachedQuestion.ask(set) { question = $0 }
            }
            switch ended {
            case .signedIn:
                result = AccountSettingsLabels.signInDone(route)
            case .cancelled:
                break
            case let .failed(detail, _):
                alert = AccountPageAlert(
                    title: AccountSettingsLabels.signInFailed(route), detail: detail)
            }
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
    /// Held as well while a browser sign-in of the account's runs.
    let held: Bool

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
                control: field.save.held(held),
                ready: !password.isEmpty
            ) {
                model.changeAccount(
                    .usePassword(account: accountId, password: TypedPassword(text: password)))
                password = ""
            }
            .buttonStyle(.borderedProminent)
        } else {
            ControlButton(
                title: L10n.settings_account_switch_to_password(),
                control: ControlState(enabled: field.save.enabled, running: false).held(held)
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
