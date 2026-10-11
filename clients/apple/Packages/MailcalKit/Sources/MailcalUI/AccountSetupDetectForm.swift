// The SwiftUI-free half of the email-first setup flow, split out of AccountSetupDetectView.swift
// so the package test suite can drive it without rendering anything: the untrusted-settings
// approval and the refused-certificate acceptance are security contracts worth covering, and
// this file is the whole of both gates. What the card offers the account for is `UseChoices`'.

import MailcalBindings

/// Connect-gating for a found card: tracks the entered secret, the untrusted-settings approval and
/// a refused certificate's acceptance, and decides whether Connect is allowed. Pure, so both gates
/// are unit-tested without SwiftUI.
struct DetectedConnectForm {
    let recommendation: SetupRecommendation
    /// No mail server was found, only a calendar or address book: the account is set up for those
    /// alone, through the same standards config an IMAP account is stored in.
    let calendarAndContacts: Bool
    /// One secret for every route: IMAP takes a password, and a JMAP server declares its auth
    /// scheme in its own 401, so a password and an API token are interchangeable here.
    var password = ""
    var approved = false
    /// The certificate the last connect was refused for, when it was refused for one. Until
    /// it is accepted, Connect stays inert: the same gate the untrusted-settings approval
    /// above uses, for the same reason (`docs/certificate-exceptions.md`).
    var rejectedCertificate: RejectedCertificate?
    var certificateAccepted = false

    init(recommendation: SetupRecommendation, calendarAndContacts: Bool = false) {
        self.recommendation = recommendation
        self.calendarAndContacts = calendarAndContacts
    }

    init(_ setup: DetectedSetup) {
        self.init(recommendation: setup.recommendation, calendarAndContacts: setup.calendarAndContacts)
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
    /// The refused certificate this route can actually do something about. Only a standards
    /// account's stored config carries an exception, so a JMAP refusal is reported and not
    /// offered: taking an answer and ignoring it is worse than not asking
    /// (`docs/certificate-exceptions.md` → Known gaps).
    var refusedCertificate: RejectedCertificate? {
        if case .imap = recommendation { return rejectedCertificate }
        return calendarAndContacts ? rejectedCertificate : nil
    }

    private var certificateOK: Bool { refusedCertificate == nil || certificateAccepted }

    /// The certificate to store with the account: the refused one, once accepted, and
    /// nothing at all otherwise.
    var acceptedCertificate: RejectedCertificate? {
        certificateAccepted ? refusedCertificate : nil
    }

    /// Whether the sign-in may start. An untrusted config decides which server the token is
    /// presented to, so it is approved first, exactly as before a typed password.
    var canSignIn: Bool { approvalOK }

    var canConnect: Bool {
        if calendarAndContacts { return !password.isEmpty && certificateOK }
        switch recommendation {
        case .jmap: return !password.isEmpty && approvalOK && certificateOK
        // On the IMAP route the password field is only on screen when the server takes one; when
        // it does not, Connect is not the action either (the sign-in button is), so gating on a
        // password would disable a button nobody is looking at.
        case .imap: return !password.isEmpty && approvalOK && certificateOK
        default: return false
        }
    }
}
