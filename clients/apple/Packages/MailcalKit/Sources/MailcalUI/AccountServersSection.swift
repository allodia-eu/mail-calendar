// An account's servers and sign-in, on its page: for an account that signs in with a password,
// whose servers are its own to change. Saving tries them first (`update_account_endpoints`), so a
// typo never costs a working account, and an expired password is replaced here too
// (docs/accounts.md rule 14).

import MailcalBindings
import SwiftUI

struct AccountServersSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let endpoints: AccountEndpoints
    let expired: Bool
    @Binding var alert: AccountPageAlert?

    /// The fields as the person is leaving them. Seeded once, from what is stored: a signal that
    /// redraws the page while somebody types must not take their typing away.
    @State private var fields: EndpointFields
    @State private var saving = false
    /// The fields as they were last saved, so "Saved." stands only while nothing has changed since.
    @State private var savedAs: EndpointFields?

    init(
        model: MailboxModel, entry: AccountEntry, endpoints: AccountEndpoints, expired: Bool,
        alert: Binding<AccountPageAlert?>
    ) {
        self.model = model
        self.entry = entry
        self.endpoints = endpoints
        self.expired = expired
        _alert = alert
        _fields = State(initialValue: EndpointFields(endpoints))
    }

    var body: some View {
        SettingsGroup(
            heading: L10n.settings_account_servers_heading(),
            description: expired
                ? L10n.signin_expired_prompt(accounts: AccountSettingsLabels.name(entry))
                : L10n.settings_account_servers_description()
        ) {
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
                        TextField(L10n.setup_hint_caldav(), text: url(\.caldav)).setupField(.host)
                    }
                }
                if fields.carddav != nil {
                    labelled(L10n.settings_account_field_carddav()) {
                        TextField(L10n.setup_hint_carddav(), text: url(\.carddav)).setupField(.host)
                    }
                }
                labelled(L10n.settings_account_field_login()) {
                    TextField(L10n.settings_account_field_login(), text: $fields.username)
                        .setupField(.email)
                }
                labelled(L10n.settings_account_field_new_password()) {
                    SecureField(L10n.settings_account_field_new_password(), text: $fields.password)
                        .setupField(.password)
                }
                HStack(spacing: 10) {
                    Button(L10n.action_save(), action: save)
                        .buttonStyle(.borderedProminent)
                        .disabled(saving || fields.incomplete)
                    if saving {
                        progress(L10n.settings_account_servers_trying())
                    } else if savedAs == fields {
                        Text(L10n.settings_account_servers_saved())
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .frame(maxWidth: 420, alignment: .leading)
        }
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

    /// Tries the edited servers, which connects to each of them. A refusal leaves the form as the
    /// person typed it, the password included, so they correct one field rather than retype them
    /// all.
    private func save() {
        let edited = fields.edited(from: endpoints)
        let id = entry.id
        saving = true
        Task {
            let failure = await model.saveAccountEndpoints(id, edited)
            saving = false
            if let failure {
                alert = AccountPageAlert(
                    title: L10n.settings_account_servers_failed(), detail: failure)
            } else {
                fields.password = ""
                savedAs = fields
            }
        }
    }
}
