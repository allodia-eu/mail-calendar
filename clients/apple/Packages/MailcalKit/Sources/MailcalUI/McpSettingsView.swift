// The Advanced ▸ AI assistant access panel (docs/mcp.md, docs/settings.md row 8).
//
// Desktop-only, and by construction rather than by an `#if`: the core reports no endpoint on a
// platform whose host set none, and the panel renders nothing when there is none. That mirrors
// Notifications being mobile-only in the same taxonomy.
//
// The order of the controls is the order of the decisions, and it is deliberate: turn it on, then
// choose which mailboxes it reaches (none to begin with), then, separately, and only if you mean
// it, let it send. Each is a distinct grant; a single switch conferring all three would be the
// wrong default in the place a wrong default costs the most.

#if canImport(AppKit)
import AppKit
#endif
import MailcalBindings
import SwiftUI

struct McpSettingsView: View {
    var model: MailboxModel

    @State private var copied = false

    var body: some View {
        if let settings = model.mcpSettings, let endpoint = settings.endpoint {
            VStack(alignment: .leading, spacing: 20) {
                masterGroup(settings)
                if settings.enabled {
                    accountsGroup(settings)
                    sendingGroup(settings)
                    configurationGroup(endpoint)
                }
            }
        }
    }

    // MARK: On/off, and whether it is actually listening

    @ViewBuilder
    private func masterGroup(_ settings: McpSettings) -> some View {
        group(L10n.settings_mcp_heading(), L10n.settings_mcp_description()) {
            Toggle(L10n.settings_mcp_toggle(), isOn: enabledBinding(settings))
            // Whether a socket is bound, not just what the switch says. The two can disagree:
            // another instance owning the endpoint, a path that will not bind, and a panel that
            // showed only the switch would tell the user it is on while nothing can reach it.
            Text(statusText(settings))
                .font(.callout)
                .foregroundStyle(settings.enabled && !settings.running ? .orange : .secondary)
        }
    }

    private func statusText(_ settings: McpSettings) -> String {
        guard settings.enabled else { return L10n.settings_mcp_status_off() }
        return settings.running
            ? L10n.settings_mcp_status_running()
            : L10n.settings_mcp_status_unavailable()
    }

    // MARK: Which mailboxes it reaches, none until the user says otherwise

    @ViewBuilder
    private func accountsGroup(_ settings: McpSettings) -> some View {
        group(
            L10n.settings_mcp_accounts_heading(),
            L10n.settings_mcp_accounts_description()
        ) {
            ForEach(settings.accounts, id: \.accountId) { account in
                Toggle(account.email, isOn: exposedBinding(account))
            }
            // Said out loud rather than implied by empty checkboxes: an assistant reporting "your
            // inbox is empty" is otherwise indistinguishable from an assistant that has been
            // given nothing to look at.
            if !settings.accounts.contains(where: \.exposed) {
                Text(L10n.settings_mcp_accounts_empty())
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: Sending, its own decision, with its own guard

    @ViewBuilder
    private func sendingGroup(_ settings: McpSettings) -> some View {
        group(L10n.settings_mcp_send_heading(), L10n.settings_mcp_send_note()) {
            Toggle(L10n.settings_mcp_send_toggle(), isOn: directSendBinding(settings))
            Toggle(
                L10n.settings_mcp_known_recipient_toggle(),
                isOn: knownRecipientBinding(settings)
            )
            // Disabled rather than hidden while direct send is off: the guard is what makes
            // direct send defensible, so the user should see it exists before they reach for the
            // switch above it.
            .disabled(!settings.allowDirectSend)
            Text(L10n.settings_mcp_known_recipient_note())
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: The snippet to paste into the assistant

    @ViewBuilder
    private func configurationGroup(_ endpoint: String) -> some View {
        group(L10n.settings_mcp_config_heading(), L10n.settings_mcp_config_description()) {
            let snippet = McpEndpoint.configurationSnippet(endpoint: endpoint)
            ScrollView(.horizontal) {
                Text(snippet)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(8)
            }
            .frame(maxHeight: 160)
            .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 6))
            Button(copied ? L10n.settings_mcp_copied() : L10n.settings_mcp_copy()) {
                copy(snippet)
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

    // MARK: Bindings

    private func enabledBinding(_ settings: McpSettings) -> Binding<Bool> {
        Binding(get: { settings.enabled }, set: { model.setMcpEnabled($0) })
    }

    private func exposedBinding(_ account: McpAccountRow) -> Binding<Bool> {
        Binding(
            get: { account.exposed },
            set: { model.setMcpAccountExposed(account.accountId, $0) }
        )
    }

    private func directSendBinding(_ settings: McpSettings) -> Binding<Bool> {
        Binding(get: { settings.allowDirectSend }, set: { model.setMcpAllowDirectSend($0) })
    }

    private func knownRecipientBinding(_ settings: McpSettings) -> Binding<Bool> {
        Binding(
            get: { settings.requireKnownRecipient },
            set: { model.setMcpRequireKnownRecipient($0) }
        )
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
