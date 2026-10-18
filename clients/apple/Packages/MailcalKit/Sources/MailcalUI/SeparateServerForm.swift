// The password a calendar or address book server of its own is asked for (docs/accounts.md rule
// 18): a step of setup after the mail sign-in (docs/onboarding.md, "After the connect: a calendar
// on a server of its own"), and an inline form on the account's page under the use that needs it.
// One set of fields for both, so the two cannot drift.

import MailcalBindings
import SwiftUI

/// The title, the server it names, the login and the password, and a failure said under them.
private struct SeparateServerFields: View {
    let ask: SeparateServerAsk
    @Binding var username: String
    @Binding var password: String
    let failure: String?
    let busy: Bool
    let connect: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(SeparateServerLabels.formBody(ask))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            TextField(L10n.setup_field_username(), text: $username)
                .setupField(.email)
                .disabled(busy)
            SecureField(L10n.setup_field_password(), text: $password)
                .setupField(.password)
                .disabled(busy)
                .onTouchKeyboardSubmit {
                    if SeparateServerRules.canConnect(username, password, busy: busy) { connect() }
                }
            if let failure {
                Text(failure).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The setup step: Skip switches the uses off, Connect moves them to an account of their own.
/// Either way setup goes on to the link step, whose pickers already show the new link.
struct SeparateServerStepView: View {
    var model: MailboxModel
    let ask: SeparateServerAsk

    @State private var username = ""
    @State private var password = ""
    @State private var failure: String?
    @State private var busy = false

    var body: some View {
        SetupScaffold {
            if let set = ask.set {
                Text(SeparateServerLabels.formTitle(set)).font(.title2).bold()
                    .fixedSize(horizontal: false, vertical: true)
            }
            SetupCard {
                SeparateServerFields(
                    ask: ask, username: $username, password: $password, failure: failure,
                    busy: busy, connect: connect)
            }
            SetupFooter {
                Button(L10n.separate_server_skip(), action: skip)
                    .keyboardShortcut(.cancelAction)
                    .disabled(busy)
                if busy {
                    ProgressView().controlSize(.small)
                } else {
                    Button(L10n.action_connect(), action: connect)
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                        .disabled(!SeparateServerRules.canConnect(username, password, busy: busy))
                }
            }
            .setupActionsInView()
        }
        .onAppear { if username.isEmpty { username = ask.username } }
    }

    private func connect() {
        guard SeparateServerRules.canConnect(username, password, busy: busy) else { return }
        busy = true
        failure = nil
        let (login, typed) = (username.trimmingCharacters(in: .whitespaces), password)
        Task {
            let refused = await model.addLinkedCalendarAccount(
                ask, username: login, password: typed)
            busy = false
            if let refused {
                failure = refused
            } else {
                model.setupSeparateServerDone()
            }
        }
    }

    private func skip() {
        busy = true
        Task {
            await model.skipSeparateServer(ask)
            busy = false
            model.setupSeparateServerDone()
        }
    }
}

/// The account page's form, opened under the use that needs it. Cancel folds it away; Connect
/// moves the uses, and the page then redraws without them and with the link.
struct AccountSeparateServerForm: View {
    var model: MailboxModel
    let ask: SeparateServerAsk
    let close: () -> Void

    @State private var username = ""
    @State private var password = ""
    @State private var failure: String?
    @State private var busy = false

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                if let set = ask.set {
                    Text(SeparateServerLabels.formTitle(set)).font(.headline)
                        .fixedSize(horizontal: false, vertical: true)
                }
                SeparateServerFields(
                    ask: ask, username: $username, password: $password, failure: failure,
                    busy: busy, connect: connect)
                    .frame(maxWidth: 320, alignment: .leading)
                HStack(spacing: 10) {
                    Button(L10n.action_connect(), action: connect)
                        .buttonStyle(.borderedProminent)
                        .disabled(!SeparateServerRules.canConnect(username, password, busy: busy))
                    Button(L10n.action_cancel(), action: close)
                        .disabled(busy)
                    if busy {
                        ProgressView().controlSize(.small)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .onAppear { if username.isEmpty { username = ask.username } }
    }

    private func connect() {
        guard SeparateServerRules.canConnect(username, password, busy: busy) else { return }
        busy = true
        failure = nil
        let (login, typed) = (username.trimmingCharacters(in: .whitespaces), password)
        Task {
            let refused = await model.addLinkedCalendarAccount(
                ask, username: login, password: typed)
            busy = false
            if let refused {
                failure = refused
            } else {
                password = ""
                close()
            }
        }
    }
}
