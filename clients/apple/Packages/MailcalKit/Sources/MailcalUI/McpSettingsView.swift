// The Advanced ▸ AI assistant access panel (docs/mcp.md, docs/settings.md, Advanced), drawn from the
// core's `McpPanel`: the status words, the "nothing exposed" note, which switch is held and the
// snippet are all the core's. Desktop-only by construction rather than by an `#if`: the core
// carries no panel on a platform whose host set no endpoint.
//
// The order of the controls is the order of the decisions: turn it on, then choose which
// mailboxes it reaches (none to begin with), then, separately, let it send.

#if canImport(AppKit)
import AppKit
#endif
import MailcalBindings
import SwiftUI

/// The words and colour of what the server is doing, from the core's `McpStatus`.
enum McpStatusWords {
    static func text(_ status: McpStatus) -> String {
        switch status {
        case .off: L10n.settings_mcp_status_off()
        case .starting: L10n.settings_mcp_status_starting()
        case .listening: L10n.settings_mcp_status_running()
        case .anotherInstance: L10n.settings_mcp_status_another_instance()
        case .unavailable: L10n.settings_mcp_status_unavailable()
        }
    }

    /// Switched on and not listening, so no assistant can reach it.
    static func warns(_ status: McpStatus) -> Bool {
        switch status {
        case .anotherInstance, .unavailable: true
        case .off, .starting, .listening: false
        }
    }
}

struct McpSettingsView: View {
    var model: MailboxModel
    let panel: McpPanel

    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            masterGroup
            if panel.enabled {
                accountsGroup
                sendingGroup
                configurationGroup
            }
        }
    }

    private var masterGroup: some View {
        group(L10n.settings_mcp_heading(), L10n.settings_mcp_description()) {
            Toggle(
                L10n.settings_mcp_toggle(),
                isOn: binding(panel.enabled) { .setEnabled(enabled: $0) })
            Text(McpStatusWords.text(panel.status))
                .font(.callout)
                .foregroundStyle(McpStatusWords.warns(panel.status) ? .orange : .secondary)
        }
    }

    private var accountsGroup: some View {
        group(
            L10n.settings_mcp_accounts_heading(),
            L10n.settings_mcp_accounts_description()
        ) {
            ForEach(panel.accounts, id: \.accountId) { account in
                Toggle(
                    account.email,
                    isOn: binding(account.exposed) {
                        .setAccountExposed(account: account.accountId, exposed: $0)
                    })
            }
            if panel.nothingExposed {
                Text(L10n.settings_mcp_accounts_empty())
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var sendingGroup: some View {
        group(L10n.settings_mcp_send_heading(), L10n.settings_mcp_send_note()) {
            Toggle(
                L10n.settings_mcp_send_toggle(),
                isOn: binding(panel.allowDirectSend) { .setDirectSend(allow: $0) })
            Toggle(
                L10n.settings_mcp_known_recipient_toggle(),
                isOn: binding(panel.requireKnownRecipient) { .setKnownRecipient(require: $0) }
            )
            .disabled(!panel.knownRecipientEditable)
            Text(L10n.settings_mcp_known_recipient_note())
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private var configurationGroup: some View {
        group(L10n.settings_mcp_config_heading(), L10n.settings_mcp_config_description()) {
            ScrollView(.horizontal) {
                Text(panel.snippet)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(8)
            }
            .frame(maxHeight: 160)
            .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 6))
            Button(copied ? L10n.settings_mcp_copied() : L10n.settings_mcp_copy()) {
                copy(panel.snippet)
            }
        }
    }

    private func copy(_ snippet: String) {
        #if canImport(AppKit)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(snippet, forType: .string)
        #endif
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            copied = false
        }
    }

    /// A switch drawn at the core's `value`, sending the change `intent` makes of it.
    private func binding(_ value: Bool, _ intent: @escaping (Bool) -> McpIntent) -> Binding<Bool> {
        Binding(get: { value }, set: { model.mcp(intent($0)) })
    }

    /// A labelled section, matching the shape the sibling settings panels use.
    @ViewBuilder
    private func group(
        _ heading: String,
        _ description: String,
        @ViewBuilder content: () -> some View
    ) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(heading).font(.headline)
                Text(description).font(.callout).foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 8) { content() }.padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}
