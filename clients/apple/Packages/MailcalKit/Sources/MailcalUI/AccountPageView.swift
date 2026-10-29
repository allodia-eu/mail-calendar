// One account's page in Settings → Accounts, in the order docs/settings.md gives: how it is shared,
// what it is used for, the accounts it relies on, how it signs in, its own servers and password,
// its mail settings while it is used for mail, and its removal, last.

import MailcalBindings
import SwiftUI

/// What the page says once something it asked for has answered. A failure is an alert, not a line
/// the person may have scrolled past.
struct AccountPageAlert: Identifiable {
    let id = UUID()
    let title: String
    let detail: String
}

struct AccountPageView: View {
    var model: MailboxModel
    let entry: AccountEntry
    /// Removes the account, and leaves the page.
    let remove: (String) -> Void

    @State private var alert: AccountPageAlert?
    /// The line the sign-in section shows once a sign-in has answered.
    @State private var signInResult: String?
    @State private var confirmingRemoval = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            header
            // What setup opened this page to say: a use the provider's sign-in withheld.
            if let notice = model.accountPageNotice, notice.account == entry.id {
                GroupBox {
                    VStack(alignment: .leading, spacing: 4) {
                        Label(notice.title, systemImage: "exclamationmark.triangle.fill")
                            .font(.headline)
                        Text(notice.detail)
                            .font(.callout)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(6)
                }
            }
            // Whether this one travels: first, because it decides whether anything below it is
            // anybody else's business (docs/settings.md).
            if let mode = model.accountsSyncMode[entry.id] {
                GroupBox {
                    AccountSyncModePicker(model: model, accountId: entry.id, mode: mode)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(6)
                }
            }
            AccountUsesSection(model: model, entry: entry, alert: $alert, signIn: signIn)
            let pickers = AccountSettingsRules.linkPickers(entry)
            if !pickers.isEmpty {
                AccountLinksSection(model: model, entry: entry, pickers: pickers, alert: $alert)
            }
            if AccountSettingsRules.signsInAtProvider(entry.kind) {
                signInSection
            } else if AccountSignInRules.switchesSignIn(entry.kind) {
                AccountSignInSection(model: model, entry: entry, alert: $alert)
            }
            if let endpoints = entry.endpoints {
                AccountServersSection(
                    model: model, entry: entry, endpoints: endpoints,
                    expired: model.signInExpired(entry.id), alert: $alert
                )
            }
            if let account = model.syncSettings?.accounts.first(where: { $0.accountId == entry.id }) {
                AccountMailSection(model: model, entry: entry, account: account, alert: $alert)
            }
            removeSection
        }
        .alert(item: $alert) { alert in
            Alert(
                title: Text(alert.title),
                message: Text(alert.detail),
                dismissButton: .cancel(Text(L10n.action_close()))
            )
        }
    }

    @ViewBuilder
    private var header: some View {
        VStack(alignment: .leading, spacing: 4) {
            #if os(macOS)
            // The page replaces the list in place on macOS, so it carries its own way back.
            Button {
                model.accountPage = nil
            } label: {
                Label(L10n.a11y_back(), systemImage: "chevron.left")
            }
            .buttonStyle(.borderless)
            .padding(.bottom, 6)
            Text(entry.address).font(.title3).bold()
            #endif
            Text(AccountSettingsLabels.kindLine(entry))
                .font(.callout)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: Sign-in

    /// "Sign in again", for an account that signs in at its provider's page: the remedy for an
    /// expired sign-in and for a permission the provider withheld.
    private var signInSection: some View {
        SettingsGroup(
            heading: L10n.settings_account_signin_heading(),
            description: AccountSettingsLabels.signIn(entry, expired: model.signInExpired(entry.id))
        ) {
            VStack(alignment: .leading, spacing: 8) {
                // Never disabled: a sign-in abandoned in the browser cannot be told from one in
                // progress, so pressing again starts a fresh one (docs/provider-oauth.md rule 13).
                Button(L10n.signin_expired_action()) {
                    signIn(adding: [])
                }
                if model.signingInAgain == entry.id {
                    progress(L10n.settings_account_signin_waiting(
                        email: AccountSettingsLabels.name(entry)))
                } else if let signInResult {
                    Text(signInResult)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    /// Signs the account in again at its provider, asking for what it is used for plus `adding`,
    /// and says how it ended. A cancelled one says nothing: the person closed it.
    private func signIn(adding: [AccountCapability]) {
        signInResult = nil
        let id = entry.id
        Task {
            switch await model.signInAgain(id, adding: adding) {
            case .signedIn:
                // A sign-in can complete while the provider still withholds a use; that is said,
                // rather than "Signed in again".
                let withheld = model.accountEntry(id).map(AccountSettingsRules.withheld) ?? []
                signInResult = withheld.isEmpty
                    ? L10n.settings_account_signed_in()
                    : L10n.setup_withheld_detail(
                        uses: withheld.map(AccountSettingsLabels.use).joined(separator: ", "))
            case .cancelled:
                break
            case let .failed(detail, _):
                alert = AccountPageAlert(title: L10n.settings_account_signin_failed(), detail: detail)
            }
        }
    }

    // MARK: Removal

    /// Last, and the only destructive control on the page: a remove sits below the things you
    /// might actually adjust, not among them.
    private var removeSection: some View {
        Button(role: .destructive) {
            confirmingRemoval = true
        } label: {
            Label(L10n.action_remove_account(), systemImage: "trash")
        }
        .buttonStyle(.bordered)
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, which
        // drops the `.cancel` button (see the remove-account alert in Mailcal.swift).
        .alert(L10n.remove_account_title(), isPresented: $confirmingRemoval) {
            Button(L10n.action_remove(), role: .destructive) { remove(entry.id) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(AccountSettingsLabels.removal(entry))
        }
    }
}

/// A switch per use the account's kind offers. Switching one off is confirmed first, because it
/// deletes what the device holds of it.
struct AccountUsesSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    @Binding var alert: AccountPageAlert?
    /// Signs the account in again, asking for these as well.
    let signIn: ([AccountCapability]) -> Void

    @State private var switching: AccountCapability?
    @State private var confirmingOff: AccountCapability?
    /// The use under which the sign-in of its own is open.
    @State private var separateServerUnder: AccountCapability?
    @State private var serverChange: ServerChangeReview?

    init(
        model: MailboxModel, entry: AccountEntry, alert: Binding<AccountPageAlert?>,
        signIn: @escaping (_ adding: [AccountCapability]) -> Void
    ) {
        self.model = model
        self.entry = entry
        _alert = alert
        self.signIn = signIn
    }

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_uses_heading(),
            description: L10n.settings_account_uses_description()
        ) {
            VStack(alignment: .leading, spacing: 10) {
                // No switch for shared mailboxes: this client has no section to choose one
                // (docs/shared-mailboxes.md §3).
                ForEach(entry.uses.filter { $0.capability != .sharedMailboxes }, id: \.capability) { use in
                    row(use.capability)
                }
            }
        }
        .alert(
            L10n.account_use_off_title(
                capability: confirmingOff.map(AccountSettingsLabels.use) ?? ""),
            isPresented: Binding(
                get: { confirmingOff != nil }, set: { if !$0 { confirmingOff = nil } }
            ),
            presenting: confirmingOff
        ) { capability in
            Button(L10n.action_switch_off(), role: .destructive) { change(capability, on: false) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: { _ in
            Text(L10n.account_use_off_message())
        }
    }

    private func row(_ capability: AccountCapability) -> some View {
        let toggle = AccountSettingsRules.useSwitch(entry, capability)
        return VStack(alignment: .leading, spacing: 2) {
            HStack {
                Toggle(
                    AccountSettingsLabels.use(capability),
                    isOn: Binding(
                        get: { toggle.active },
                        set: { on in flipped(capability, on: on, asks: toggle.asks) }
                    )
                )
                // A switch, as the copy calls it ("Switch off Contacts?").
                .toggleStyle(.switch)
                .disabled(!toggle.enabled || switching != nil)
                if switching == capability {
                    ProgressView().controlSize(.small)
                }
            }
            if let note = AccountSettingsLabels.note(toggle.note) {
                Text(note)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if AccountSettingsRules.reviewsServerChange(toggle.note) {
                Button(L10n.server_changed_review()) {
                    serverChange = model.serverChangeReview(entry.id)
                }
                .controlSize(.small)
            }
            separateServer(capability)
        }
    }

    /// A sign-in of its own, under a use whose server needs one or refused the account's
    /// (docs/accounts.md rule 18): a button, then the form it opens.
    @ViewBuilder
    private func separateServer(_ capability: AccountCapability) -> some View {
        if SeparateServerRules.offered(entry, capability),
           let ask = SeparateServerRules.ask(entry)
        {
            if separateServerUnder == capability {
                AccountSeparateServerForm(model: model, ask: ask) { separateServerUnder = nil }
                    .padding(.top, 4)
            } else {
                Button(L10n.settings_account_use_separate_sign_in()) {
                    separateServerUnder = capability
                }
                .controlSize(.small)
                .disabled(separateServerUnder != nil || switching != nil)
            }
        }
    }

    private func flipped(_ capability: AccountCapability, on: Bool, asks: Bool) {
        if on, asks {
            // Already chosen, and waiting on the provider: switching it on asks again.
            signIn([capability])
        } else if on {
            change(capability, on: true)
        } else {
            confirmingOff = capability
        }
    }

    /// Asks the core. A use the provider has not granted is asked for straight away: the person
    /// switching it on is the person who would be asked.
    private func change(_ capability: AccountCapability, on: Bool) {
        switching = capability
        let id = entry.id
        Task {
            let outcome = await model.setAccountUse(id, capability, on: on)
            switching = nil
            switch outcome {
            case .applied:
                break
            case .askProvider:
                signIn([capability])
            case .needsEndpoint:
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(),
                    detail: L10n.settings_account_use_needs_endpoint())
            case let .failed(detail):
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(), detail: detail)
            }
        }
    }
}

/// A picker per link slot the account can hold, a suggested account named under it with a button
/// that links it: the person confirms a suggestion, it is never linked for them
/// (docs/accounts.md rule 12).
struct AccountLinksSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let pickers: [LinkPicker]
    @Binding var alert: AccountPageAlert?

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_links_heading(),
            description: L10n.settings_account_links_description()
        ) {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(pickers, id: \.slot) { picker in
                    LinkPickerRow(picker: picker) { target in link(picker.slot, target) }
                }
            }
        }
    }

    private func link(_ slot: LinkSlot, _ target: String?) {
        let id = entry.id
        Task {
            if let failure = await model.setAccountLink(id, slot, target) {
                alert = AccountPageAlert(
                    title: L10n.settings_account_change_failed_title(), detail: failure)
            }
        }
    }
}

/// One link slot: its heading, a picker with "None" first, and the suggestion beneath it. Shared
/// with the link step at the end of setup, which offers exactly what this page would.
struct LinkPickerRow: View {
    let picker: LinkPicker
    /// The option picked, as its index into `picker.options`; `nil` for "None". The page links on
    /// it, the setup step records it.
    var selected: Int?
    /// Whether the suggestion is named with a button that picks it. The setup step starts with
    /// the suggestion already picked, so it names it without one.
    var suggestButton = true
    let pick: (String?) -> Void

    init(
        picker: LinkPicker, selected: Int?? = nil, suggestButton: Bool = true,
        pick: @escaping (String?) -> Void
    ) {
        self.picker = picker
        self.selected = selected ?? picker.selected
        self.suggestButton = suggestButton
        self.pick = pick
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Drawn here rather than left to the Picker: outside a Form, a `.menu` Picker renders
            // its label on macOS but drops it on iOS.
            Text(AccountSettingsLabels.slot(picker.slot)).font(.subheadline).bold()
            Picker(
                AccountSettingsLabels.slot(picker.slot),
                selection: Binding<String?>(
                    get: { selected.map { picker.options[$0].id } },
                    set: { pick($0) }
                )
            ) {
                Text(L10n.settings_account_link_none()).tag(String?.none)
                ForEach(picker.options, id: \.id) { option in
                    Text(AccountSettingsLabels.name(option)).tag(Optional(option.id))
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .fixedSize()
            if let suggested = picker.suggested {
                let option = picker.options[suggested]
                HStack(spacing: 8) {
                    Text(L10n.settings_account_link_suggested(
                        address: AccountSettingsLabels.name(option)))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if suggestButton, selected != suggested {
                        Button(L10n.action_link()) { pick(option.id) }
                            .controlSize(.small)
                    }
                }
            }
        }
    }
}
