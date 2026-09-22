import MailcalBindings

/// One server's connection security and port on the manual setup form.
///
/// The port starts at the standard one for the chosen security and follows the picker, until the
/// user types a port of their own; from then on it is theirs and the picker leaves it alone. A
/// server on a port nobody standardised is the whole reason the manual form exists, so the form
/// must never take back what was typed for it.
///
/// The rule is `docs/account-autodetect.md`'s and binds every client; this is the Apple client's
/// copy of it, renderer-free so it can be tested without a view.
struct ManualServerField: Equatable {
    let kind: MailServerKind
    private(set) var security: ConnectionSecurity
    private(set) var port: String
    private var typedByHand: Bool

    init(_ kind: MailServerKind) {
        self.kind = kind
        self.security = .implicitTls
        self.port = String(standardPort(kind: kind, security: .implicitTls))
        self.typedByHand = false
    }

    /// Whether the picker may still move the port.
    var followsSecurity: Bool { !typedByHand }

    /// The user picked a security. The port follows only while it is still ours.
    mutating func choose(_ security: ConnectionSecurity) {
        self.security = security
        if !typedByHand {
            port = String(standardPort(kind: kind, security: security))
        }
    }

    /// The user typed in the port field. Clearing it hands the port back to the picker: an empty
    /// field submits a bare host, which the core resolves to the same standard port, so there is
    /// nothing else a cleared field could mean.
    mutating func typePort(_ typed: String) {
        typedByHand = !typed.trimmingCharacters(in: .whitespaces).isEmpty
        port = typedByHand ? typed : String(standardPort(kind: kind, security: security))
    }

    /// A detected route filled this server in. Its port travels inside the host (`host:port`), so
    /// the field shows what detection found and stops following the picker.
    mutating func adoptDetected(host: String, security: ConnectionSecurity) {
        self.security = security
        let found = ManualServerField.splitHost(host).port
        typedByHand = !found.isEmpty
        port = typedByHand ? found : String(standardPort(kind: kind, security: security))
    }

    /// The `host:port` this field submits, or the bare host when it carries no port of its own.
    func dial(_ host: String) -> String {
        let typed = host.trimmingCharacters(in: .whitespaces)
        // A host the user already wrote a port into wins: two ports would be a contradiction, and
        // the one in the host field is the one they can see beside the name.
        if typed.isEmpty || !ManualServerField.splitHost(typed).port.isEmpty {
            return typed
        }
        let chosen = port.trimmingCharacters(in: .whitespaces)
        return chosen.isEmpty ? typed : "\(typed):\(chosen)"
    }

    /// Splits a typed server into host and port.
    ///
    /// Mirrors the core's own split (`mailcal-account`'s `host_and_addr`) deliberately: the core
    /// decides the address actually dialled, so a client that split differently would show one
    /// port and connect to another. A bare IPv6 literal is not a server either side accepts.
    static func splitHost(_ input: String) -> (host: String, port: String) {
        guard let at = input.lastIndex(of: ":") else { return (input, "") }
        let head = String(input[input.startIndex..<at])
        let tail = String(input[input.index(after: at)...])
        guard !head.isEmpty, !tail.isEmpty, tail.allSatisfy(\.isASCII), tail.allSatisfy(\.isNumber)
        else {
            return (input, "")
        }
        return (head, tail)
    }
}

/// An account's two servers, so the form carries one value rather than four.
struct ManualServerPair: Equatable {
    var imap = ManualServerField(.imap)
    var smtp = ManualServerField(.smtp)
}
