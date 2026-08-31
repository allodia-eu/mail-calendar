// The usage-statistics controls, shared by the welcome screen and both settings surfaces (the
// categorised macOS screen and the simpler iOS sheet). Shared so the three cannot drift: they show
// the same payload and write through the same setter. Their Android twin is AnalyticsConsentUi.kt:
// keep the wording and the rules in step.
//
// GDPR Art. 7(3): withdrawing consent must be as easy as giving it. That is why the settings
// control is the same one-tap toggle the welcome screen offered, not a buried confirmation flow.

import MailcalBindings
import SwiftUI

/// "See exactly what we send", and behind it the literal bytes.
///
/// The JSON comes from the core's own payload type, the same one the sink serializes, so this is
/// the payload, not a description of it. It is pulled lazily: an unopened panel costs nothing.
///
/// Monospaced and scrolled sideways rather than wrapped. A re-flowed payload is a paraphrase, and
/// the entire point of this panel is that it is not one.
struct AnalyticsPayloadPanel: View {
    @Binding var isExpanded: Bool
    let payloadPreview: () -> String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                isExpanded.toggle()
            } label: {
                Label(
                    L10n.welcome_analytics_preview(),
                    systemImage: isExpanded ? "chevron.down" : "chevron.right"
                )
                .font(.callout)
            }
            .buttonStyle(.plain)
            .foregroundStyle(.tint)

            if isExpanded {
                ScrollView(.horizontal) {
                    Text(payloadPreview())
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                        .padding(10)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Settings → Privacy: the live toggle, and the same payload panel the welcome screen showed.
/// Turning it off deletes the install id locally and asks the backend to erase what it holds.
struct AnalyticsConsentControl: View {
    var model: MailboxModel
    @State private var showingPayload = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle(L10n.settings_analytics_toggle(), isOn: binding)
            AnalyticsPayloadPanel(
                isExpanded: $showingPayload,
                payloadPreview: { model.analyticsPayloadPreview() }
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var binding: Binding<Bool> {
        Binding(
            get: { model.analyticsConsent?.enabled ?? false },
            set: { model.setAnalyticsConsent($0) }
        )
    }
}
