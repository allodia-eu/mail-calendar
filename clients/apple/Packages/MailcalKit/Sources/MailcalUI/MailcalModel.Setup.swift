// The setup-form flows on the model: the manual IMAP/JMAP submit paths and email-based
// detection. Factored into its own `MailboxModel` extension to keep MailcalModel.swift under the
// 500-line limit (the model is already split this way, see MailcalModel.Actions.swift,
// MailcalModel.Composer.swift, …).

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Builds the config from the setup-form fields, connects it as a new account, and (on
    /// success) stores it in the Keychain. Used both for the first account (full-screen
    /// form) and adding another (sheet). Blank optional fields are omitted; an invalid
    /// config or failed connect surfaces as `setupError` and keeps the form up.
    func submitSetup(
        imapHost: String,
        username: String,
        password: String,
        smtpHost: String,
        caldavBaseUrl: String,
        imapSecurity: ConnectionSecurity = .implicitTls,
        smtpSecurity: ConnectionSecurity = .implicitTls
    ) {
        let setup = AccountSetup(
            imapHost: imapHost,
            username: username,
            password: password,
            smtpHost: smtpHost.isEmpty ? nil : smtpHost,
            caldavBaseUrl: caldavBaseUrl.isEmpty ? nil : caldavBaseUrl,
            // The manual form uses the implicit-TLS defaults; the detected path passes the
            // security detection found, so the engine dials implicit TLS or STARTTLS to match.
            imapSecurity: imapSecurity,
            smtpSecurity: smtpSecurity
        )
        // The app is built (account-less) before the form is shown, so this normally holds.
        // It fails only if the engine itself couldn't open at launch, surface that rather
        // than letting the Connect button silently do nothing.
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return
        }
        let configToml: String
        do {
            configToml = try accountConfigToml(setup: setup)
        } catch {
            setupError = "\(error)"
            return
        }
        // `addAccount` blocks on the IMAP login + first sync, so run it OFF the main thread:
        // otherwise the whole UI freezes and the spinner can't animate. Flip `isConnecting`
        // around it so the Connect button shows progress and can't be fired twice. Persist only
        // after it connects, so a bad config is never stored.
        setupError = nil
        isConnecting = true
        Task { @MainActor in
            defer { self.isConnecting = false }
            do {
                _ = try await Task.detached(priority: .userInitiated) {
                    try app.addAccount(configToml: configToml)
                }.value
                if let calendarError = app.calendarConnectError() {
                    print("[Mailcal] calendar (CalDAV) failed to connect: \(calendarError)")
                }
                self.accountWasAdded()
            } catch {
                self.setupError = "\(error)"
            }
        }
    }

    /// Builds a JMAP config from the setup-form fields, connects it as a new account, and (on
    /// success) stores it in the Keychain. The server may be blank (the core derives it from
    /// the email domain). One secret, a password or an API token, stored the same way, since
    /// the engine negotiates the scheme from the server's own challenge. Mirrors `submitSetup`:
    /// same off-main connect + persist-after-connect discipline (JMAP is HTTP Basic/bearer,
    /// so it needs no browser flow, unlike Microsoft).
    func submitJmapSetup(
        email: String,
        serverURL: String,
        password: String
    ) {
        let setup = JmapSetup(
            email: email,
            serverUrl: serverURL.isEmpty ? nil : serverURL,
            password: password
        )
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return
        }
        let configToml: String
        do {
            configToml = try jmapAccountConfigToml(setup: setup)
        } catch {
            setupError = "\(error)"
            return
        }
        // `addAndStoreJmapAccount` blocks on the JMAP connect + first sync off the main thread and
        // persists only after it connects (so a bad config is never stored), exactly as
        // `submitSetup` does for IMAP. It is shared with the OAuth sign-in route
        // (MailcalModel.Jmap.swift), which produces the same config TOML, one add + store path
        // for both. `isConnecting` around it keeps the Connect button honest.
        setupError = nil
        isConnecting = true
        Task { @MainActor in
            defer { self.isConnecting = false }
            do {
                try await self.addAndStoreJmapAccount(app, configToml: configToml)
            } catch {
                self.setupError = "\(error)"
            }
        }
    }

    /// Detects a provider's settings from just the email address, off the main thread (the
    /// core call blocks up to ~10 s). The device's own DNS answers the MX fallback via
    /// `SystemMxResolver`. Returns a manual fallback if the app somehow isn't up (the setup
    /// flow only shows when it is).
    func detectSetup(email: String) async -> SetupRecommendation {
        guard let app else { return .manual(reason: .networkError) }
        return await Task.detached(priority: .userInitiated) {
            app.detectAccountSettings(email: email, mxResolver: SystemMxResolver())
        }.value
    }
}
