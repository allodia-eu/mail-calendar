// Showcase (screenshot) boot: brings the app up on the in-memory showcase dataset instead of the
// Keychain's real accounts, so no personal mail can appear in a store screenshot. Split out of
// MailcalModel.swift to keep that file under 500 lines. Gated by `ShowcaseMode.isOn`, which is
// hard-`false` in a release build.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// If `MAILCAL_SHOWCASE` is set, boot the seeded in-memory showcase app and return `true` so
    /// `start()` returns early. Nothing is persisted and no network is touched: the mailbox and
    /// calendar are served from bundled sample content, seeded in the language the chrome renders.
    /// Returns `false` when the flag is unset, the caller then takes its normal account path.
    func startShowcaseIfRequested(observer: SurfaceObserver) -> Bool {
        guard ShowcaseMode.isOn else { return false }
        let locale = ShowcaseMode.seedLocale
        logAppleLifecycle("MAILCAL_SHOWCASE set, bringing up the in-memory \(locale) showcase dataset")
        let app = MailcalApp.newShowcase(
            observer: observer,
            logger: CoreLogger(),
            logLevel: DiagnosticsPrefs.coreLogLevel,
            deviceTimezone: deviceTimeZone(),
            locale: locale
        )
        self.app = app
        self.timezone = app.timezoneSettings()
        needsSetup = false
        // The agent (MCP) surface. Without these three lines Settings → Advanced renders *nothing*
        // for agent access, `McpSettingsView` draws only when the core has both settings and an
        // endpoint, so the documentation captures came back showing Reset Database alone, on a
        // pane that looked otherwise healthy. Every automated check passed it; only looking at the
        // PNG did not.
        //
        // Its own data-directory name, never `DevNamespace`'s: a screenshot run must not bind, or
        // adopt, a socket a real build on this machine is serving on. Nothing is persisted in this
        // boot, so whatever a capture switches on lives only for that launch.
        app.setAgentHostUi(ui: AgentComposerBridge(model: self))
        app.setMcpEndpoint(endpoint: McpEndpoint.path(dataDirName: "mailcal-showcase"))
        mcpSettings = app.mcpSettings()
        app.dispatch(intent: .refreshMail)
        return true
    }
}
