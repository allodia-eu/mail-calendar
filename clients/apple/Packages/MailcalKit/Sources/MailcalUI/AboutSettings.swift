// Settings → About: which release this is, where to ask for help, and whose work it is built on.
// The content is the core's (`aboutInfo`) so every client says the same thing, a support answer
// that names a version has to name the same version on every platform; only the labels around it
// are the catalog's. Its twins are Linux's settings/about.rs, Android's SettingsAbout.kt and
// Windows' SettingsDialog.About.cs.
//
// Its own file, like DiagnosticsSettings, so SettingsCategoryDetail stays readable.

import MailcalBindings
import SwiftUI

/// The About detail, shared by the macOS sidebar, the iPad split and the iPhone hub.
struct AboutSettingsView: View {
    // The Apple frameworks this app draws with are the operating system's own, so `.apple`
    // attributes the shared core and nothing else.
    private let about = aboutInfo(platform: .apple)

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            group(L10n.app_title(), L10n.about_version(version: about.version)) {
                EmptyView()
            }
            group(L10n.about_support_heading(), L10n.about_support_description()) {
                Text(about.supportUrl).textSelection(.enabled)
                if let url = URL(string: about.supportUrl) {
                    // Opens in the user's browser: this app's only web views are the locked-down
                    // reading and composing islands (docs/rendering-security.md), not a browser.
                    Link(L10n.about_support_action(), destination: url)
                }
            }
            group(L10n.about_attributions_heading(), L10n.about_attributions_description()) {
                ForEach(about.attributions, id: \.name) { item in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(item.name).textSelection(.enabled)
                        Text(item.license).font(.callout).foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                }
            }
        }
    }

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
                content().padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}
