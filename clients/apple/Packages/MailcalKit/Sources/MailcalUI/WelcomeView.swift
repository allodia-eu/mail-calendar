// The first-boot welcome screen, and the one place we ask about usage statistics. Shared by macOS,
// iOS and iPadOS, the question, the copy and the default are identical on every platform, and a
// per-platform copy of this view is exactly how they would stop being identical.
//
// The rules it implements are legal conditions, not styling choices (docs/analytics.md):
//
//   * The toggle starts **off**, and nothing is written to the device until the user leaves the
//     screen. Under ePrivacy Art. 5(3) the *act* of storing the install id is what needs consent,
//     so a pre-ticked toggle would not merely be rude, it would be the violation itself.
//   * Refusing costs nothing. "Get started" is the only way forward and it is always enabled, so
//     the toggle is genuinely optional rather than a toll gate.
//   * The consent is unbundled: this screen accepts no terms and creates no account. It asks one
//     question and takes one answer (GDPR Art. 7(2)).
//   * "See exactly what we send" renders the literal payload the core would put on the wire.

import MailcalBindings
import SwiftUI

struct WelcomeView: View {
    /// The literal JSON the core would send, pulled lazily, only when the user asks to see it.
    let payloadPreview: () -> String
    /// The user's decision, taken exactly once when they leave the screen: `true` only if they
    /// deliberately turned the toggle on. Recording the `false` case matters as much as the `true`
    /// one, it is what stops us asking again.
    let getStarted: (Bool) -> Void

    // Default OFF. The whole feature hangs off this one line.
    @State private var shareStats = false
    @State private var showingPayload = false

    var body: some View {
        VStack(spacing: 0) {
            // `minHeight: proxy.size.height` is what centres it: a ScrollView sizes its content to
            // itself, so without a floor there is no free space to centre within. Given the floor,
            // the frame's default .centre alignment does the work, and content that outgrows the
            // screen (the payload panel, a large accessibility font) simply exceeds the floor and
            // scrolls as before.
            GeometryReader { proxy in
                ScrollView {
                    VStack(spacing: 12) {
                        Image("WelcomeArt", bundle: .module)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 140, height: 140)
                            .accessibilityLabel(L10n.a11y_welcome_art())

                        Text(L10n.welcome_title())
                            .font(.largeTitle).bold()
                            .multilineTextAlignment(.center)
                        Text(L10n.welcome_tagline())
                            .font(.title3)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .padding(.bottom, 12)

                        consentCard

                        Link(L10n.welcome_privacy_policy(), destination: privacyURL)
                            .font(.callout)
                            .padding(.top, 4)
                    }
                    .frame(maxWidth: 460)
                    .padding(24)
                    .frame(maxWidth: .infinity, minHeight: proxy.size.height)
                }
            }

            // Outside the scroll view, so it stays on screen however far the payload panel pushes
            // the content down, and enabled whichever way the toggle is set.
            Button(L10n.welcome_get_started()) { getStarted(shareStats) }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .keyboardShortcut(.defaultAction)
                .padding(24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private var consentCard: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Toggle(L10n.welcome_analytics_toggle(), isOn: $shareStats)
                    .font(.headline)
                Text(L10n.welcome_analytics_body())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                AnalyticsPayloadPanel(
                    isExpanded: $showingPayload,
                    payloadPreview: payloadPreview
                )
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }

    /// The catalog carries the URL, so all four clients point at one place and a localised policy
    /// page can diverge later without touching any client.
    private var privacyURL: URL {
        URL(string: L10n.welcome_privacy_url()) ?? URL(string: "https://allodia.eu")!
    }
}
