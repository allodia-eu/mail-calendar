// The sign-in a calendar or address book server of its own is asked for (docs/accounts.md rule
// 18): a step of setup after the mail sign-in (docs/onboarding.md, "After the connect: a calendar
// on a server of its own"), and an inline form on the account's page under the use that needs it.
// One set of fields for both, so the two cannot drift. Both draw the core's section, which the
// model reads again on every signal; the form ends once the core draws none. Where that server's
// own provider or Nextcloud may sign it in (docs/mail-oauth.md rule 13), the sign-in leads and
// the password waits behind it. Leaving the form cancels a sign-in the browser still holds.

import MailcalBindings
import SwiftUI

/// The server it names, the sign-in once offered, the login and the password, and a failure said
/// under them.
private struct SeparateServerFields: View {
    var model: MailboxModel
    @Bindable var form: SeparateServerFormModel
    let ask: SeparateServerAsk
    var takesReturn: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(SeparateServerLabels.formBody(ask))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if let way = form.signIn.offers(ask) {
                SeparateServerSignInButton(
                    model: model, form: form, ask: ask, way: way, takesReturn: takesReturn)
            }
            if form.signIn.showsPasswordFields(ask) {
                // Held while the core holds Connect: a change runs, or a sign-in is out.
                HeldFields(control: form.section.connect) {
                    TextField(L10n.setup_field_username(), text: $form.username)
                        .setupField(.email)
                    SecureField(L10n.setup_field_password(), text: $form.password)
                        .setupField(.password)
                        .onTouchKeyboardSubmit { form.connect(model, ask) }
                }
            }
            if let said = form.said {
                Text(said).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onChange(of: ask, initial: true) { _, drawn in form.drawn(drawn) }
    }
}

/// The setup step: Skip switches the uses off, Connect or the sign-in moves them to an account of
/// their own. Either way the core draws no section after it, and setup goes on to the link step
/// (`MailboxModel.refreshSeparateServers`), whose pickers already show the new link.
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
                SeparateServerFields(model: model, form: form, ask: ask, takesReturn: true)
            }
            SetupFooter {
                ControlButton(
                    title: L10n.separate_server_skip(), control: form.section.stopUsing,
                    shortcut: .cancelAction
                ) { form.skip(model, ask) }
                if form.signIn.showsPasswordFields(ask) {
                    ControlButton(
                        title: L10n.action_connect(), control: form.section.connect,
                        ready: form.canConnect, shortcut: .defaultAction
                    ) { form.connect(model, ask) }
                    .buttonStyle(.borderedProminent)
                }
            }
            .setupActionsInView()
        }
        .onDisappear {
            form.leave(model)
            model.separateServers[.setup] = nil
        }
    }
}

/// The account page's form, opened under the use the core names. Cancel folds it away; Connect or
/// the sign-in moves the uses, and the page then redraws without the form and with the link.
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
                SeparateServerFields(model: model, form: form, ask: ask, takesReturn: false)
                    .frame(maxWidth: 320, alignment: .leading)
                HStack(spacing: 10) {
                    if form.signIn.showsPasswordFields(ask) {
                        ControlButton(
                            title: L10n.action_connect(), control: form.section.connect,
                            ready: form.canConnect
                        ) { form.connect(model, ask) }
                        .buttonStyle(.borderedProminent)
                    }
                    // It folds the form away, and leaving it cancels a sign-in still out.
                    ControlButton(title: L10n.action_cancel(), control: .free(form.canLeave)) {
                        close()
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .onDisappear {
            form.leave(model)
            close()
        }
    }
}
