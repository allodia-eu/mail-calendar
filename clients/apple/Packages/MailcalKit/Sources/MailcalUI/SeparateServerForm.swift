// The sign-in a calendar or address book server of its own is asked for (docs/accounts.md rule
// 18): a step of setup after the mail sign-in (docs/onboarding.md, "After the connect: a calendar
// on a server of its own"), and an inline form on the account's page under the use that needs it.
// One set of fields for both, so the two cannot drift. Where that server's own provider may sign
// it in (docs/mail-oauth.md rule 13), the sign-in leads and the password waits behind it.

import MailcalBindings
import SwiftUI

/// The server it names, the sign-in once offered, the login and the password, and a failure said
/// under them.
private struct SeparateServerFields: View {
    var model: MailboxModel
    @Bindable var form: SeparateServerFormModel
    let ask: SeparateServerAsk
    var takesReturn: Bool
    let done: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(SeparateServerLabels.formBody(ask))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if form.signIn.offersSignIn(ask) {
                SeparateServerSignInButton(form: form, ask: ask, takesReturn: takesReturn) {
                    form.startSignIn(model, ask, done: done)
                }
            }
            if form.signIn.showsPasswordFields(ask) {
                TextField(L10n.setup_field_username(), text: $form.username)
                    .setupField(.email)
                    .disabled(form.holding)
                SecureField(L10n.setup_field_password(), text: $form.password)
                    .setupField(.password)
                    .disabled(form.holding)
                    .onTouchKeyboardSubmit { form.connect(model, ask, done: done) }
            }
            if let failure = form.failure {
                Text(failure).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        // Asked once the form is drawn; until it answers, the form is the password form.
        .task(id: ask) { await form.askOffer(model, ask) }
    }
}

/// The setup step: Skip switches the uses off, Connect or the sign-in moves them to an account of
/// their own. Either way setup goes on to the link step, whose pickers already show the new link.
struct SeparateServerStepView: View {
    var model: MailboxModel
    let ask: SeparateServerAsk

    @State private var form: SeparateServerFormModel

    init(model: MailboxModel, ask: SeparateServerAsk) {
        self.model = model
        self.ask = ask
        _form = State(initialValue: SeparateServerFormModel(ask))
    }

    var body: some View {
        SetupScaffold {
            if let set = ask.set {
                Text(SeparateServerLabels.formTitle(set)).font(.title2).bold()
                    .fixedSize(horizontal: false, vertical: true)
            }
            SetupCard {
                SeparateServerFields(
                    model: model, form: form, ask: ask, takesReturn: true,
                    done: { model.setupSeparateServerDone() })
            }
            SetupFooter {
                Button(L10n.separate_server_skip(), action: skip)
                    .keyboardShortcut(.cancelAction)
                    .disabled(!form.canLeave)
                if form.busy {
                    ProgressView().controlSize(.small)
                } else if form.signIn.showsPasswordFields(ask) {
                    Button(L10n.action_connect()) {
                        form.connect(model, ask) { model.setupSeparateServerDone() }
                    }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!form.canConnect)
                }
            }
            .setupActionsInView()
        }
        .onDisappear { form.retire() }
    }

    private func skip() {
        guard form.canLeave else { return }
        form.retire()
        form.busy = true
        Task {
            await model.skipSeparateServer(ask)
            form.busy = false
            model.setupSeparateServerDone()
        }
    }
}

/// The account page's form, opened under the use that needs it. Cancel folds it away; Connect or
/// the sign-in moves the uses, and the page then redraws without them and with the link.
struct AccountSeparateServerForm: View {
    var model: MailboxModel
    let ask: SeparateServerAsk
    let close: () -> Void

    @State private var form: SeparateServerFormModel

    init(model: MailboxModel, ask: SeparateServerAsk, close: @escaping () -> Void) {
        self.model = model
        self.ask = ask
        self.close = close
        _form = State(initialValue: SeparateServerFormModel(ask))
    }

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                if let set = ask.set {
                    Text(SeparateServerLabels.formTitle(set)).font(.headline)
                        .fixedSize(horizontal: false, vertical: true)
                }
                SeparateServerFields(
                    model: model, form: form, ask: ask, takesReturn: false, done: moved)
                    .frame(maxWidth: 320, alignment: .leading)
                HStack(spacing: 10) {
                    if form.signIn.showsPasswordFields(ask) {
                        Button(L10n.action_connect()) { form.connect(model, ask, done: moved) }
                            .buttonStyle(.borderedProminent)
                            .disabled(!form.canConnect)
                    }
                    Button(L10n.action_cancel()) {
                        form.retire()
                        close()
                    }
                    .disabled(!form.canLeave)
                    if form.busy {
                        ProgressView().controlSize(.small)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .onDisappear { form.retire() }
    }

    private func moved() {
        form.password = ""
        close()
    }
}
