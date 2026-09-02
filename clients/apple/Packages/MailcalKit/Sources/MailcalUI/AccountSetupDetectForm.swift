// The pure halves of the detection flow: what Connect is allowed to do with a result, and how
// the card asks a mail server what it accepts.
//
// Split from AccountSetupDetectView, which had reached the size limit, along a seam worth having
// anyway: none of this is SwiftUI, so the approval gate (a security contract) and the pre-flight
// are driven by the package test suite without a view.

import SwiftUI
import MailcalBindings

/// Connect-gating for a JMAP/IMAP detection result: tracks the entered secret and the
/// untrusted-settings approval, and decides whether Connect is allowed. Pure, so the
/// approval gate (a security contract) is unit-tested without SwiftUI.
struct DetectedConnectForm {
    let recommendation: SetupRecommendation
    /// One secret for both routes: IMAP takes a password, and a JMAP server declares its auth
    /// scheme in its own 401, so a password and an API token are interchangeable here.
    var password = ""
    var approved = false
    /// Calendar (IMAP only). Defaults ON when detection discovered a CalDAV endpoint
    /// (opt-out), OFF otherwise (opt-in); either way it reuses the IMAP credentials.
    var calendarEnabled: Bool
    var calendarURLEntry = ""

    init(recommendation: SetupRecommendation) {
        self.recommendation = recommendation
        self.calendarEnabled = Self.discoveredCaldav(recommendation) != nil
    }

    var isTrusted: Bool {
        switch recommendation {
        case let .jmap(_, _, isTrusted, _): return isTrusted
        case let .imap(_, _, _, _, _, _, _, _, _, isTrusted, _): return isTrusted
        default: return true
        }
    }

    var needsApproval: Bool { !isTrusted }
    private var approvalOK: Bool { isTrusted || approved }

    var canConnect: Bool {
        switch recommendation {
        case .jmap: return !password.isEmpty && approvalOK
        // On the IMAP route the password field is only on screen when the server takes one; when
        // it does not, Connect is not the action either (the sign-in button is), so gating on a
        // password would disable a button nobody is looking at.
        case .imap: return !password.isEmpty && approvalOK
        default: return false
        }
    }

    /// The CalDAV endpoint detection discovered for this account, if any.
    var discoveredCaldav: String? { Self.discoveredCaldav(recommendation) }

    private static func discoveredCaldav(_ recommendation: SetupRecommendation) -> String? {
        if case let .imap(_, _, _, _, _, _, _, caldavURL, _, _, _) = recommendation { return caldavURL }
        return nil
    }

    /// The CalDAV URL to store: the discovered endpoint, else a manually entered one; nil
    /// when calendar is switched off or nothing was entered.
    var effectiveCaldavURL: String? {
        guard calendarEnabled else { return nil }
        return discoveredCaldav ?? (calendarURLEntry.isEmpty ? nil : calendarURLEntry)
    }
}
