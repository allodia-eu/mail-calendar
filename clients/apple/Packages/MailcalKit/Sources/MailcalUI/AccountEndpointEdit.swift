// An account's servers and sign-in as its page edits them (docs/accounts.md rule 14). Saving tries
// them first (`update_account_endpoints`), so a typo never costs a working account. Renderer-free
// so the arithmetic is tested without a view.

import Foundation
import MailcalBindings

/// One mail server's fields on an account's page: its host, and its port and security through the
/// setup form's own `ManualServerField`, so the port follows the security exactly as it does there.
struct EndpointServerFields: Equatable {
    var host: String
    var field: ManualServerField

    /// The fields for a stored address, `host` or `host:port`.
    init(_ kind: MailServerKind, stored: String, security: ConnectionSecurity) {
        host = ManualServerField.splitHost(stored).host
        field = ManualServerField(kind)
        field.adoptDetected(host: stored, security: security)
    }

    /// The address to store: the bare host when the port is the standard one for the security,
    /// `host:port` otherwise. A port written into the host field is kept, never doubled.
    var stored: String {
        let dialled = field.dial(host)
        let (bare, port) = ManualServerField.splitHost(dialled)
        let standard = String(standardPort(kind: field.kind, security: field.security))
        return port == standard ? bare : dialled
    }
}

/// The page's fields as the person left them.
struct EndpointFields: Equatable {
    var imap: EndpointServerFields?
    var smtp: EndpointServerFields?
    var caldav: String?
    var carddav: String?
    var username: String
    var password = ""

    /// The fields for `endpoints`, as stored.
    init(_ endpoints: AccountEndpoints) {
        imap = endpoints.imapHost.map {
            EndpointServerFields(.imap, stored: $0, security: endpoints.imapSecurity)
        }
        smtp = endpoints.smtpHost.map {
            EndpointServerFields(.smtp, stored: $0, security: endpoints.smtpSecurity)
        }
        caldav = endpoints.caldavUrl
        carddav = endpoints.carddavUrl
        username = endpoints.username
    }

    /// Whether a field the account has is empty. Saving waits: an empty server would leave its use
    /// without one, which the core refuses only after the person has been made to wait for it.
    var incomplete: Bool {
        let blank = { (text: String?) in
            text.map { $0.trimmingCharacters(in: .whitespaces).isEmpty } ?? false
        }
        return blank(imap?.host) || blank(smtp?.host) || blank(caldav) || blank(carddav)
            || blank(username)
    }

    /// The servers to save: `original` with what the fields changed. A server the account does
    /// not have stays absent, and an empty password keeps the stored one.
    func edited(from original: AccountEndpoints) -> AccountEndpoints {
        func kept(_ old: String?, _ typed: String?) -> String? {
            old.map { old in typed?.trimmingCharacters(in: .whitespaces) ?? old }
        }
        return AccountEndpoints(
            imapHost: kept(original.imapHost, imap?.stored),
            imapSecurity: imap?.field.security ?? original.imapSecurity,
            smtpHost: kept(original.smtpHost, smtp?.stored),
            smtpSecurity: smtp?.field.security ?? original.smtpSecurity,
            caldavUrl: kept(original.caldavUrl, caldav),
            carddavUrl: kept(original.carddavUrl, carddav),
            username: username.trimmingCharacters(in: .whitespaces),
            password: password.isEmpty ? nil : password
        )
    }
}
