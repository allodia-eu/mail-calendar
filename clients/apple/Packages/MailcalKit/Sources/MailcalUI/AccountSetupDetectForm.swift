// The SwiftUI-free half of the email-first setup flow, split out of AccountSetupDetectView.swift
// so the package test suite can drive it without rendering anything: the untrusted-settings
// approval and the refused-certificate acceptance are security contracts worth covering, and
// this file is the whole of both gates.

import MailcalBindings

/// Connect-gating for a JMAP/IMAP detection result: tracks the entered secret, the
/// untrusted-settings approval and a refused certificate's acceptance, and decides whether
/// Connect is allowed. Pure, so both gates are unit-tested without SwiftUI.
struct DetectedConnectForm {
    let recommendation: SetupRecommendation
    /// One secret for both routes: IMAP takes a password, and a JMAP server declares its auth
    /// scheme in its own 401, so a password and an API token are interchangeable here.
    var password = ""
    var approved = false
    /// The certificate the last connect was refused for, when it was refused for one. Until
    /// it is accepted, Connect stays inert: the same gate the untrusted-settings approval
    /// above uses, for the same reason (`docs/certificate-exceptions.md`).
    var rejectedCertificate: RejectedCertificate?
    var certificateAccepted = false
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
        case let .imap(_, _, _, _, _, _, _, _, isTrusted, _): return isTrusted
        default: return true
        }
    }

    var needsApproval: Bool { !isTrusted }
    private var approvalOK: Bool { isTrusted || approved }
    /// The refused certificate this route can actually do something about. Only an IMAP
    /// account's stored config carries an exception, so a JMAP refusal is reported and not
    /// offered: taking an answer and ignoring it is worse than not asking
    /// (`docs/certificate-exceptions.md` → Known gaps).
    var refusedCertificate: RejectedCertificate? {
        if case .imap = recommendation { return rejectedCertificate }
        return nil
    }

    private var certificateOK: Bool { refusedCertificate == nil || certificateAccepted }

    /// The certificate to store with the account: the refused one, once accepted, and
    /// nothing at all otherwise.
    var acceptedCertificate: RejectedCertificate? {
        certificateAccepted ? refusedCertificate : nil
    }

    var canConnect: Bool {
        switch recommendation {
        case .jmap: return !password.isEmpty && approvalOK && certificateOK
        case .imap: return !password.isEmpty && approvalOK && certificateOK
        default: return false
        }
    }

    /// The CalDAV endpoint detection discovered for this account, if any.
    var discoveredCaldav: String? { Self.discoveredCaldav(recommendation) }

    private static func discoveredCaldav(_ recommendation: SetupRecommendation) -> String? {
        if case let .imap(_, _, _, _, _, _, _, caldavURL, _, _) = recommendation { return caldavURL }
        return nil
    }

    /// The CalDAV URL to store: the discovered endpoint, else a manually entered one; nil
    /// when calendar is switched off or nothing was entered.
    var effectiveCaldavURL: String? {
        guard calendarEnabled else { return nil }
        return discoveredCaldav ?? (calendarURLEntry.isEmpty ? nil : calendarURLEntry)
    }
}
