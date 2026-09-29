// The parts of the email-first setup flow that hold no view state: what the manual form is
// prefilled with, the lines that explain a result, and the small views every step shares.
//
// Split from AccountSetupDetectView.swift, which had reached the size limit; nothing here reads
// the view's state, so it moves without widening anything the view keeps private.

import SwiftUI
import MailcalBindings

/// The host of a discovered URL (CalDAV endpoint, JMAP base), for a compact confirmation line,
/// so an untrusted result's "check the server names" has a name to check; the full URL is the
/// fallback if it somehow doesn't parse.
func urlHost(_ url: String) -> String {
    URLComponents(string: url)?.host ?? url
}

/// A running step's readout: a small spinner and what it is waiting for.
func progress(_ label: String) -> some View {
    HStack(spacing: 8) {
        ProgressView().controlSize(.small)
        Text(label).foregroundStyle(.secondary)
    }
}

/// What to prefill in the manual form when the user edits a discovered config.
struct ManualPrefill {
    var kind: AccountKind = .imap
    var email = ""
    var imapHost = ""
    var smtpHost = ""
    var jmapServer = ""
}

func manualPrefill(_ edit: SetupRecommendation?, typedEmail: String) -> ManualPrefill {
    switch edit {
    case let .imap(email, imapHost, smtpHost, _, _, _, _, _, _, _, _):
        return ManualPrefill(kind: .imap, email: email, imapHost: imapHost, smtpHost: smtpHost ?? "")
    case let .jmap(email, serverURL, _, _):
        return ManualPrefill(kind: .jmap, email: email, jmapServer: serverURL)
    case let .microsoft(email):
        return ManualPrefill(kind: .microsoft, email: email)
    case let .google(email):
        return ManualPrefill(kind: .google, email: email)
    default:
        return ManualPrefill(email: typedEmail)
    }
}

/// The localised line explaining why detection sent the user to manual setup.
func reasonNote(_ reason: MissReason) -> String {
    switch reason {
    case .networkError: return L10n.setup_detect_reason_network()
    case .oauthOnlyProvider: return L10n.setup_detect_reason_oauth_only()
    case .nothingFound, .invalidEmail: return L10n.setup_detect_reason_nothing()
    }
}
