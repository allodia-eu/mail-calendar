// An account's servers and sign-in, on its page: for an account that signs in with a password,
// whose servers are its own to change. Saving tries them first (`SaveServers`), so a typo never
// costs a working account, and an expired password is replaced here too (docs/accounts.md rule
// 14).

import MailcalBindings
import SwiftUI

struct AccountServersSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let servers: AccountServers

    /// The fields as the person is leaving them, seeded from the page's form: the edit being saved
    /// while a save runs, the one the last save refused, and otherwise what is stored. Seeded again
    /// only when that changes, so a signal that redraws the page leaves typing alone.
    @State private var fields: EndpointFields

    init(model: MailboxModel, entry: AccountEntry, servers: AccountServers) {
        self.model = model
        self.entry = entry
        self.servers = servers
        _fields = State(initialValue: EndpointFields(servers.form))
    }

    var body: some View {
        let edited = fields.edited(from: servers.form)
        let gap = model.endpointsGap(entry.id, edited)
        SettingsGroup(
            heading: L10n.settings_account_servers_heading(),
            description: servers.signInRefused
                ? L10n.signin_expired_prompt(accounts: AccountSettingsLabels.name(entry))
                : L10n.settings_account_servers_description()
        ) {
            VStack(alignment: .leading, spacing: 12) {
                HeldFields(control: servers.save) {
                    VStack(alignment: .leading, spacing: 12) {
                        if fields.imap != nil {
                            labelled(L10n.settings_account_field_imap()) {
                                ManualServerRow(
                                    placeholder: L10n.setup_hint_imap(),
                                    host: server(\.imap).host, field: server(\.imap).field)
                            }
                        }
                        if fields.smtp != nil {
                            labelled(L10n.setup_field_smtp()) {
                                ManualServerRow(
                                    placeholder: L10n.setup_hint_smtp(),
                                    host: server(\.smtp).host, field: server(\.smtp).field)
                            }
                        }
                        if fields.caldav != nil {
                            labelled(L10n.setup_field_caldav()) {
                                TextField(L10n.setup_hint_caldav(), text: url(\.caldav))
                                    .setupField(.host)
                            }
                        }
                        if fields.carddav != nil {
                            labelled(L10n.settings_account_field_carddav()) {
                                TextField(L10n.setup_hint_carddav(), text: url(\.carddav))
                                    .setupField(.host)
                            }
                        }
                        labelled(L10n.settings_account_field_login()) {
                            TextField(L10n.settings_account_field_login(), text: $fields.username)
                                .setupField(.email)
                        }
                        labelled(L10n.settings_account_field_new_password()) {
                            SecureField(
                                L10n.settings_account_field_new_password(), text: $fields.password
                            )
                            .setupField(.password)
                        }
                    }
                }
                if let gap {
                    Text(AccountSettingsLabels.endpointsGap(gap))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                ControlButton(
                    title: L10n.action_save(), control: servers.save, ready: gap == nil
                ) {
                    model.changeAccount(.saveServers(account: entry.id, endpoints: edited))
                }
                .buttonStyle(.borderedProminent)
            }
            .frame(maxWidth: 420, alignment: .leading)
        }
        .onChange(of: servers.form) { _, form in fields = EndpointFields(form) }
    }

    private func labelled<Content: View>(
        _ title: String, @ViewBuilder _ content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            content()
        }
    }

    /// A binding into one server's fields, which exist exactly when the account has that server.
    private func server(
        _ path: WritableKeyPath<EndpointFields, EndpointServerFields?>
    ) -> Binding<EndpointServerFields> {
        Binding(
            get: { fields[keyPath: path]! },
            set: { fields[keyPath: path] = $0 }
        )
    }

    private func url(_ path: WritableKeyPath<EndpointFields, String?>) -> Binding<String> {
        Binding(get: { fields[keyPath: path] ?? "" }, set: { fields[keyPath: path] = $0 })
    }

}
