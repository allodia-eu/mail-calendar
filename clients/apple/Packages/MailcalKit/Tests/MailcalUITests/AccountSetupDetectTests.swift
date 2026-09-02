// The detection connect-form gating, especially the untrusted-settings approval, a
// cross-platform security contract. Pure logic, no SwiftUI.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

struct AccountSetupDetectTests {
    private func jmap(_ isTrusted: Bool) -> SetupRecommendation {
        .jmap(
            email: "alice@example.com",
            serverUrl: "https://example.com",
            isTrusted: isTrusted,
            source: "https://example.com/.well-known/jmap"
        )
    }

    private func imap(_ isTrusted: Bool, withSmtp: Bool = true, caldavURL: String? = nil) -> SetupRecommendation {
        .imap(
            email: "alice@example.com",
            imapHost: "imap.example.com",
            smtpHost: withSmtp ? "smtp.example.com" : nil,
            imapSecurity: .implicitTls,
            smtpSecurity: .implicitTls,
            incoming: DetectedServerRow(
                protocol: "IMAP", hostname: "imap.example.com", port: 993,
                security: "SSL/TLS", username: "alice@example.com"
            ),
            outgoing: withSmtp
                ? DetectedServerRow(
                    protocol: "SMTP", hostname: "smtp.example.com", port: 465,
                    security: "SSL/TLS", username: "alice@example.com"
                )
                : nil,
            caldavUrl: caldavURL,
            oauthIssuer: nil,
            isTrusted: isTrusted,
            source: "https://autoconfig.example.com/mail/config-v1.1.xml"
        )
    }

    @Test func jmapConnectsWithASecret() {
        var form = DetectedConnectForm(recommendation: jmap(true))
        #expect(!form.canConnect)
        form.password = "secret"
        #expect(form.canConnect)
    }

    @Test func jmapTakesAnApiTokenInTheSameSecretField() {
        // One field, either kind of secret: the detected card no longer asks the user to
        // classify what their provider issued them.
        var form = DetectedConnectForm(recommendation: jmap(true))
        form.password = "api-token"
        #expect(form.canConnect)
    }

    @Test func untrustedJmapRequiresApproval() {
        var form = DetectedConnectForm(recommendation: jmap(false))
        form.password = "secret"
        #expect(form.needsApproval)
        #expect(!form.canConnect)
        form.approved = true
        #expect(form.canConnect)
    }

    @Test func imapConnectsWithAPassword() {
        var form = DetectedConnectForm(recommendation: imap(true))
        #expect(!form.needsApproval)
        #expect(!form.canConnect)
        form.password = "hunter2"
        #expect(form.canConnect)
    }

    @Test func untrustedImapRequiresApproval() {
        var form = DetectedConnectForm(recommendation: imap(false))
        form.password = "hunter2"
        #expect(!form.canConnect)
        form.approved = true
        #expect(form.canConnect)
    }

    @Test func oauthAndManualNeverConnectDirectly() {
        // OAuth providers (Microsoft, Google) sign in via the browser, not this connect form, so
        // the form never enables Connect for them, nor for a manual-fallback recommendation.
        #expect(!DetectedConnectForm(recommendation: .microsoft(email: "a@b.com")).canConnect)
        #expect(!DetectedConnectForm(recommendation: .google(email: "a@b.com")).canConnect)
        #expect(!DetectedConnectForm(recommendation: .manual(reason: .nothingFound)).canConnect)
    }

    @Test func aDiscoveredCalendarIsPreselectedAndRidesTheCredentials() {
        // A found endpoint is opt-out: pre-checked, and stored on connect reusing the password.
        var form = DetectedConnectForm(recommendation: imap(true, caldavURL: "https://caldav.soverin.net/calendars"))
        #expect(form.calendarEnabled)
        form.password = "hunter2"
        #expect(form.effectiveCaldavURL == "https://caldav.soverin.net/calendars")
    }

    @Test func optingOutOfADiscoveredCalendarLeavesItUnset() {
        var form = DetectedConnectForm(recommendation: imap(true, caldavURL: "https://caldav.soverin.net/calendars"))
        form.calendarEnabled = false
        #expect(form.effectiveCaldavURL == nil)
    }

    @Test func withoutADiscoveredCalendarTheUserCanAddOneManually() {
        // Nothing discovered → opt-in: off by default until the user turns it on and types a URL.
        var form = DetectedConnectForm(recommendation: imap(true, caldavURL: nil))
        #expect(!form.calendarEnabled)
        form.calendarEnabled = true
        form.calendarURLEntry = "caldav.example.com"
        #expect(form.effectiveCaldavURL == "caldav.example.com")
    }

    // What the setup card asks for once the server has answered. Three states rather than a flag,
    // and the middle one is the reason: a provider whose sign-in is closed to this application is
    // not the same as one that offers none.

    @Test func nothingIsAskedForWhileTheServerIsStillBeingAsked() {
        // A credential field that appears and is then taken away reads as the app changing its
        // mind, and the answer decides whether it belongs there at all.
        let state = ImapAuthState.checking
        #expect(!state.showsPassword)
        #expect(!state.offersSignIn)
    }

    @Test func aProviderOfferingSignInKeepsThePasswordRouteWhereItWorks() {
        let state = ImapAuthState(
            .signIn(issuer: "https://login.example.com", providerLabel: nil, passwordAlsoWorks: true)
        )
        #expect(state.offersSignIn)
        #expect(state.showsPassword)
    }

    @Test func aServerThatRefusesPasswordsIsNotOfferedAPasswordField() {
        // Microsoft 365's shape: OAuth only. That field would be a dead end nobody finds until
        // they have typed one into it.
        let state = ImapAuthState(
            .signIn(issuer: "https://login.example.com", providerLabel: nil, passwordAlsoWorks: false)
        )
        #expect(state.offersSignIn)
        #expect(!state.showsPassword)
    }

    @Test func aClosedSignInStillLeadsToThePasswordField() {
        // The explanation is what differs from `.password`; the route offered is the same one.
        let state = ImapAuthState(.registrationNeeded(passwordAlsoWorks: true))
        #expect(!state.offersSignIn)
        #expect(state.showsPassword)
    }

    @Test func aFailedSignInBringsThePasswordFieldBack() {
        // It is the route left, so it must be there, and the line beside it says why.
        #expect(ImapAuthState.failed.showsPassword)
        #expect(!ImapAuthState.failed.offersSignIn)
    }
}
