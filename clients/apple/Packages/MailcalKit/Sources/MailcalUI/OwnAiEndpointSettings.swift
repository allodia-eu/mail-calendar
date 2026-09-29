// Settings → Advanced → Own AI endpoint (docs/ai.md, "Where requests go"; docs/settings.md row 11):
// any server that speaks OpenAI's chat API, on every platform, and what makes Writing style appear
// in a build without Allodia's relay. The address, model and declaration are preferences; the key is
// in the Keychain and never comes back, so the field says whether one is stored instead.

import Foundation
import MailcalBindings
import SwiftUI

struct OwnAiEndpointSettings: View {
    var model: MailboxModel

    @State private var stored: OwnAiEndpoint?
    @State private var address = ""
    @State private var key = ""
    @State private var modelName = ""
    /// Where the person says it runs; `nil` until they say, which the gate treats as unknown.
    @State private var declared: JurisdictionClass?
    @State private var error: String?

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.ai_endpoint_title()).font(.headline)
                Text(L10n.ai_endpoint_intro())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                VStack(alignment: .leading, spacing: 10) {
                    field(L10n.ai_endpoint_address(), hint: L10n.ai_endpoint_address_hint()) {
                        TextField(L10n.ai_endpoint_address(), text: $address)
                            .fieldConfig(.host)
                    }
                    field(
                        L10n.ai_endpoint_key(),
                        hint: stored?.hasKey == true
                            ? L10n.ai_endpoint_key_stored()
                            : L10n.ai_endpoint_key_hint()
                    ) {
                        SecureField(L10n.ai_endpoint_key(), text: $key)
                            .fieldConfig(.password)
                    }
                    field(L10n.ai_endpoint_model(), hint: nil) {
                        TextField(L10n.ai_endpoint_model(), text: $modelName)
                            .fieldConfig(.host)
                    }
                    VStack(alignment: .leading, spacing: 4) {
                        Text(L10n.ai_endpoint_where()).font(.subheadline).bold()
                        ChoiceList(title: L10n.ai_endpoint_where(), selection: $declared, options: [
                            (.euNative, L10n.ai_endpoint_where_eu_native()),
                            (.euHosted, L10n.ai_endpoint_where_eu_hosted()),
                            (.nonEu, L10n.ai_endpoint_where_non_eu()),
                        ])
                    }
                    if let error {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    HStack(spacing: 10) {
                        Button(L10n.ai_endpoint_save()) { save() }
                            .buttonStyle(.bordered)
                        if stored != nil {
                            Button(L10n.ai_endpoint_remove(), role: .destructive) { remove() }
                                .buttonStyle(.bordered)
                        }
                    }
                    .controlSize(.small)
                }
                .padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .task { load() }
    }

    private func field(
        _ label: String, hint: String?, @ViewBuilder content: () -> some View
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.subheadline).bold()
            content()
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 420, alignment: .leading)
            if let hint {
                Text(hint)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func load() {
        stored = model.ownAiEndpoint()
        address = stored?.baseUrl ?? ""
        modelName = stored?.model ?? ""
        declared = stored?.declared
        key = ""
    }

    private func save() {
        let failure = model.setOwnAiEndpoint(
            baseUrl: address.trimmingCharacters(in: .whitespacesAndNewlines),
            key: ownEndpointKeyArgument(key),
            model: modelName.trimmingCharacters(in: .whitespacesAndNewlines),
            declared: declared
        )
        error = failure.map(ownEndpointErrorText)
        if failure == nil { load() }
    }

    private func remove() {
        error = model.clearOwnAiEndpoint().map(ownEndpointErrorText)
        load()
    }
}
