// One account's page in Settings → Accounts, in the order docs/settings.md gives: how it is shared,
// the name it sends under, what it is used for, the accounts it relies on and those relying on it,
// its shared mailboxes, how it signs in, its own servers and password, its mail settings while it
// is used for mail, and its removal, last.

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
    /// Its uses, links and removal, as the core decided them.
    let page: AccountPage
    /// Removes the account; the page goes once the account is gone.
    let remove: (String) -> Void

    @State private var alert: AccountPageAlert?
    @State private var confirmingRemoval: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            header
            // What the core says is wrong with this account, at the top of its page, each with
            // its remedy (docs/notices.md rule 6).
            ForEach(model.notices(about: entry.id), id: \.id) { notice in
                GroupBox {
                    NoticeEntry(model: model, notice: notice)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(6)
                }
            }
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
            if let name = page.senderName {
                SenderNameSection(model: model, accountId: entry.id, name: name)
            }
            AccountUsesSection(model: model, entry: entry, uses: page.uses)
            if !page.links.isEmpty {
                AccountLinksSection(model: model, entry: entry, pickers: page.links)
            }
            if !page.linkedFrom.isEmpty {
                AccountLinkedFromSection(model: model, entry: entry, linked: page.linkedFrom)
            }
            if let replies = page.replyFallback {
                AccountReplyFallbackSection(model: model, entry: entry, picker: replies)
            }
            if let shared = page.shared {
                AccountSharedMailboxesSection(model: model, entry: entry, shared: shared)
            }
            if let section = page.signIn {
                AccountSignInSection(model: model, entry: entry, section: section)
            }
            if let technology = entry.mailSync {
                AccountSyncTechnologySection(technology: technology)
            }
            if let servers = page.servers {
                AccountServersSection(model: model, entry: entry, servers: servers)
            }
            if let mail = page.mail {
                AccountMailSection(model: model, entry: entry, mail: mail)
            }
            removeSection
        }
        // Its server is asked again which mailboxes the account's sign-in opens.
        .task(id: entry.id) { model.accountPageOpened(entry.id) }
        // What a change on this page came to, said once (docs/waiting.md rule 5).
        .onChange(of: model.accountPageState.failure?.id, initial: true) {
            guard let failure = model.accountPageState.failure else { return }
            model.accountPageState.failure = nil
            alert = failure
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

    // MARK: Removal

    /// Last, and the only destructive control on the page: a remove sits below the things you
    /// might actually adjust, not among them.
    private var removeSection: some View {
        ControlButton(
            title: L10n.action_remove_account(), systemImage: "trash", role: .destructive,
            control: page.remove
        ) {
            confirmingRemoval = entry.id
        }
        .buttonStyle(.bordered)
        .confirmAlert(
            $confirmingRemoval, action: L10n.action_remove(),
            title: { _ in L10n.remove_account_title() },
            message: { _ in AccountSettingsLabels.removal(entry) },
            confirm: remove)
    }
}

/// A switch per use the account's kind offers, as the core decided each, or for a use a link
/// fills, the account it comes from (docs/accounts.md rule 12). Switching one off is confirmed
/// first where it deletes what the device holds of it; switching on one the provider withholds
/// goes to its sign-in, asking for it.
struct AccountUsesSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let uses: [AccountUseSwitch]

    @State private var confirmingOff: AccountCapability?
    @State private var serverChange: ServerChangeReview?

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_uses_heading(),
            description: L10n.settings_account_uses_description()
        ) {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(uses, id: \.capability) { toggle in
                    row(toggle)
                }
            }
        }
        .confirmAlert(
            $confirmingOff, action: L10n.action_switch_off(),
            title: { L10n.account_use_off_title(capability: AccountSettingsLabels.use($0)) },
            message: { _ in L10n.account_use_off_message() },
            confirm: { set($0, on: false) })
        // The Review under a use paused for a new server asks the question here, as the outage
        // banner does (docs/account-autodetect.md rule 14).
        .serverChangeReview(model, $serverChange)
    }

    @ViewBuilder
    private func row(_ toggle: AccountUseSwitch) -> some View {
        if let linked = LinkedUseRow.of(toggle) {
            ControlNavRow(title: linked.title, subtitle: linked.note, enabled: linked.enabled) {
                model.accountPage = linked.opens
            }
        } else {
            useSwitch(toggle)
        }
    }

    private func useSwitch(_ toggle: AccountUseSwitch) -> some View {
        let capability = toggle.capability
        let control = toggle.control
        return VStack(alignment: .leading, spacing: 2) {
            ControlToggle(
                title: AccountSettingsLabels.use(capability),
                note: AccountSettingsLabels.note(toggle),
                on: toggle.on,
                control: control
            ) { on in
                if on, toggle.asksProvider {
                    model.signInThroughBrowser(entry.id, adding: [capability])
                } else if !on, toggle.confirmsOff {
                    confirmingOff = capability
                } else {
                    set(capability, on: on)
                }
            }
            if toggle.reviewsServerChange {
                // It opens a question and starts nothing, so it is only held while a change runs.
                ControlButton(title: L10n.server_changed_review(), control: .free(control.enabled)) {
                    serverChange = model.serverChangeReview(entry.id)
                }
                .controlSize(.small)
            }
            if toggle.signsInSeparately {
                separateServer(enabled: control.enabled)
            }
        }
    }

    /// A sign-in of its own, under the use the core names (docs/accounts.md rule 18): a button,
    /// then the form it opens, which closes once the core draws no section.
    @ViewBuilder
    private func separateServer(enabled: Bool) -> some View {
        if let ask = model.pageSeparateServer(entry.id) {
            AccountSeparateServerForm(model: model, ask: ask) { model.separateServers[.page] = nil }
                .padding(.top, 4)
        } else {
            // It opens a form and starts nothing, so it is only held while a change runs.
            ControlButton(
                title: L10n.settings_account_use_separate_sign_in(), control: .free(enabled)
            ) {
                model.openSeparateServer(entry.id)
            }
            .controlSize(.small)
        }
    }

    private func set(_ capability: AccountCapability, on: Bool) {
        model.changeAccount(.setUse(account: entry.id, capability: capability, on: on))
    }
}

/// A picker per link slot the account can hold, a suggested account named under it with a button
/// that links it: the person confirms a suggestion, it is never linked for them
/// (docs/accounts.md rule 12).
struct AccountLinksSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let pickers: [AccountLinkPicker]

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_links_heading(),
            description: L10n.settings_account_links_description()
        ) {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(pickers, id: \.slot) { picker in
                    LinkPickerRow(picker: picker) { target in
                        model.changeAccount(
                            .setLink(account: entry.id, slot: picker.slot, target: target))
                    }
                }
            }
        }
    }
}

/// What happens to an invitation answer the account's calendar server cannot pass on: asked each
/// time with a notice, emailed, or not (docs/invitations.md).
struct AccountReplyFallbackSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let picker: AccountReplyFallback

    private static let choices: [ReplyFallbackChoice] = [.ask, .always, .never]

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_reply_fallback_heading(),
            description: L10n.settings_reply_fallback_description()
        ) {
            ControlPicker(
                title: L10n.settings_reply_fallback_label(),
                labels: [
                    L10n.settings_reply_fallback_ask(),
                    L10n.settings_reply_fallback_always(),
                    L10n.settings_reply_fallback_never(),
                ],
                selected: Self.choices.firstIndex(of: picker.choice) ?? 0,
                control: picker.control
            ) { at in
                model.changeAccount(.setReplyFallback(account: entry.id, choice: Self.choices[at]))
            }
        }
    }
}

/// One link slot: its picker with "None" first, and the suggestion beneath it. Shared with the
/// link step at the end of setup, which offers the slots this page would that nothing fills yet.
struct LinkPickerRow: View {
    let picker: AccountLinkPicker
    /// The option picked, as its index into `picker.options`; `nil` for "None". The page links on
    /// it, the setup step records it.
    var selected: Int?
    /// Whether the suggestion is named with a button that picks it. The setup step starts with
    /// the suggestion already picked, so it names it without one.
    var suggestButton = true
    /// Setup's own hold, which the core does not know about; the page's is the picker's.
    var control: ControlState
    let pick: (String?) -> Void

    init(
        picker: AccountLinkPicker, selected: Int?? = nil, suggestButton: Bool = true,
        control: ControlState? = nil, pick: @escaping (String?) -> Void
    ) {
        self.picker = picker
        self.selected = selected ?? picker.selected.map { Int($0) }
        self.suggestButton = suggestButton
        self.control = control ?? picker.control
        self.pick = pick
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ControlPicker(
                title: AccountSettingsLabels.slot(picker.slot),
                labels: [L10n.settings_account_link_none()]
                    + picker.options.map { AccountSettingsLabels.name($0) },
                selected: selected.map { $0 + 1 } ?? 0,
                control: control
            ) { at in
                pick(at == 0 ? nil : picker.options[at - 1].id)
            }
            if let suggested = picker.suggested.map({ Int($0) }) {
                let option = picker.options[suggested]
                HStack(spacing: 8) {
                    Text(L10n.settings_account_link_suggested(
                        address: AccountSettingsLabels.name(option)))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if suggestButton, selected != suggested {
                        // The picker beside it shows the link running.
                        ControlButton(title: L10n.action_link(), control: .free(control.enabled)) {
                            pick(option.id)
                        }
                        .controlSize(.small)
                    }
                }
            }
        }
    }
}
