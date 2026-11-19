// Settings → Advanced → Own AI endpoint (docs/ai.md rules 6 and 7), drawn from the core's
// `ai_endpoint_form`: nothing while the feature is not offered. What a save would refuse is asked
// of `ai_endpoint_gap` as the person types and said beside its field, Save held meanwhile; Save
// and Remove are changes on the record, running until the key is in the Keychain and the rest in
// the preferences. A kept key is never drawn back.

import MailcalBindings
import SwiftUI

struct AiEndpointSettings: View {
    var model: MailboxModel

    var body: some View {
        if let form = model.aiEndpoint {
            AiEndpointFormView(model: model, form: form)
        }
    }
}

private struct AiEndpointFormView: View {
    var model: MailboxModel
    let form: AiEndpointForm

    /// Seeded from the record, and again only when what it draws moves or an outcome says so, so
    /// a signal that redraws the page leaves typing alone.
    @State private var fields: AiEndpointFields
    @State private var session: AiEndpointSession
    /// Why the last change did not take, when the fields' own lines do not say it.
    @State private var problem: String?

    init(model: MailboxModel, form: AiEndpointForm) {
        self.model = model
        self.form = form
        _fields = State(initialValue: AiEndpointFields(form))
        _session = State(initialValue: AiEndpointSession(form: form))
    }

    var body: some View {
        let gaps = model.aiEndpointGap(fields)
        // A form nobody has started on names no gap: Save is held, and three lines saying what is
        // missing would read as something gone wrong.
        let untouched = fields == AiEndpointFields(form) && fields.address.isEmpty
            && fields.model.isEmpty && fields.declared == nil
        SettingsGroup(heading: L10n.ai_endpoint_title(), description: L10n.ai_endpoint_intro()) {
            VStack(alignment: .leading, spacing: 12) {
                HeldFields(control: form.save) {
                    VStack(alignment: .leading, spacing: 12) {
                        field(
                            L10n.ai_endpoint_address(), hint: L10n.ai_endpoint_address_hint(),
                            gap: untouched ? nil : gaps.address.map(AiWords.addressGap)
                        ) {
                            TextField(L10n.ai_endpoint_address(), text: $fields.address)
                                .fieldConfig(.host)
                                .accessibilityIdentifier("aiEndpoint.address")
                        }
                        field(
                            L10n.ai_endpoint_key(),
                            hint: AiWords.keyHint(gaps.keptKey),
                            gap: nil
                        ) {
                            SecureField(L10n.ai_endpoint_key(), text: $fields.key)
                                .fieldConfig(.password)
                                .accessibilityIdentifier("aiEndpoint.key")
                        }
                        field(
                            L10n.ai_endpoint_model(), hint: nil,
                            gap: untouched ? nil : AiWords.modelGap(gaps)
                        ) {
                            TextField(L10n.ai_endpoint_model(), text: $fields.model)
                                .fieldConfig(.host)
                                .accessibilityIdentifier("aiEndpoint.model")
                        }
                        declaration(gap: untouched ? nil : gaps.declaration)
                    }
                }
                if let problem {
                    Text(problem)
                        .font(.callout)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("aiEndpoint.problem")
                }
                HStack(spacing: 12) {
                    ControlButton(
                        title: L10n.ai_endpoint_save(), control: form.save, ready: !gaps.any
                    ) {
                        problem = nil
                        model.saveAiEndpoint(fields)
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("aiEndpoint.save")
                    if let remove = form.remove {
                        ControlButton(
                            title: L10n.ai_endpoint_remove(), role: .destructive, control: remove
                        ) {
                            problem = nil
                            model.removeAiEndpoint()
                        }
                        .buttonStyle(.bordered)
                        .accessibilityIdentifier("aiEndpoint.remove")
                    }
                }
            }
            .frame(maxWidth: 460, alignment: .leading)
        }
        .onChange(of: AiEndpointFields(form)) { _, drawn in
            var next = drawn
            next.key = fields.key
            fields = next
        }
        .onChange(of: form.outcome?.serial) { _, _ in
            switch session.take(form.outcome) {
            case .adopt:
                fields = AiEndpointFields(form)
            case let .keep(said):
                problem = said
                if let said { AccessibilityNotification.Announcement(said).post() }
            case nil:
                break
            }
        }
    }

    /// Where it runs: one of three, or none until the person says.
    private func declaration(gap: AiEndpointDeclarationGap?) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(L10n.ai_endpoint_where()).font(.subheadline).bold()
            #if os(macOS)
            Picker(L10n.ai_endpoint_where(), selection: $fields.declared) {
                ForEach(AiWords.declarations, id: \.0) { declared, title in
                    Text(title).tag(JurisdictionClass?.some(declared))
                }
            }
            .pickerStyle(.radioGroup)
            .labelsHidden()
            #else
            // A checkmark list, as iOS Settings draws a choice of one: the three answers are
            // sentences, which a menu would cut short and an inline picker draws as a wheel.
            VStack(alignment: .leading, spacing: 0) {
                ForEach(AiWords.declarations, id: \.0) { declared, title in
                    Button {
                        fields.declared = declared
                    } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Text(title)
                                .foregroundStyle(.primary)
                                .multilineTextAlignment(.leading)
                                .fixedSize(horizontal: false, vertical: true)
                            Spacer(minLength: 0)
                            Image(systemName: "checkmark")
                                .foregroundStyle(.tint)
                                .opacity(fields.declared == declared ? 1 : 0)
                                .accessibilityHidden(true)
                        }
                        .frame(minHeight: 44)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(fields.declared == declared ? .isSelected : [])
                }
            }
            #endif
            if let gap {
                gapLine(AiWords.declarationGap(gap))
            }
        }
    }

    private func field(
        _ label: String, hint: String?, gap: String?, @ViewBuilder content: () -> some View
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.subheadline).bold()
            content()
                .textFieldStyle(.roundedBorder)
            if let gap {
                gapLine(gap)
            } else if let hint {
                Text(hint)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func gapLine(_ text: String) -> some View {
        Label(text, systemImage: "exclamationmark.circle")
            .font(.caption)
            .foregroundStyle(.orange)
            .fixedSize(horizontal: false, vertical: true)
    }
}
