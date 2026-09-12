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
}
