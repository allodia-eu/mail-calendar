//! One server's connection security and port on the manual setup form.
//!
//! The port starts at the standard one for the chosen security and follows the picker, until the
//! user types a port of their own; from then on it is theirs and the picker leaves it alone. A
//! server on a port nobody standardised is the whole reason the manual form exists, so the form
//! must never take back what was typed for it.
//!
//! The rule is `docs/account-autodetect.md`'s and binds every client; this is the GTK client's
//! copy of it, renderer-free so it can be tested without a widget tree.

use mailcal_bindings::{
    ConnectionSecurity, MailServerKind, server_address, server_address_parts, standard_port,
};

#[derive(Clone, Debug)]
pub(crate) struct ServerField {
    kind: MailServerKind,
    security: ConnectionSecurity,
    port: String,
    typed_by_hand: bool,
}

impl ServerField {
    pub(crate) fn new(kind: MailServerKind) -> Self {
        let security = ConnectionSecurity::ImplicitTls;
        Self {
            kind,
            security,
            port: standard_port(kind, security).to_string(),
            typed_by_hand: false,
        }
    }

    pub(crate) fn kind(&self) -> MailServerKind {
        self.kind
    }

    pub(crate) fn security(&self) -> ConnectionSecurity {
        self.security
    }

    pub(crate) fn port(&self) -> &str {
        &self.port
    }

    /// Whether the picker may still move the port.
    pub(crate) fn follows_security(&self) -> bool {
        !self.typed_by_hand
    }

    /// The user picked a security. The port follows only while it is still ours.
    pub(crate) fn choose_security(&mut self, security: ConnectionSecurity) {
        self.security = security;
        if !self.typed_by_hand {
            self.port = standard_port(self.kind, security).to_string();
        }
    }

    /// The user typed in the port field. Clearing it hands the port back to the picker: an empty
    /// field submits a bare host, which the core resolves to the same standard port, so there is
    /// nothing else a cleared field could mean.
    pub(crate) fn type_port(&mut self, port: &str) {
        self.typed_by_hand = !port.trim().is_empty();
        self.port = if self.typed_by_hand {
            port.to_owned()
        } else {
            standard_port(self.kind, self.security).to_string()
        };
    }

    /// A detected route filled this server in. Its port travels inside the host (`host:port`), so
    /// the field shows what detection found and stops following the picker.
    pub(crate) fn adopt_detected(&mut self, host: &str, security: ConnectionSecurity) {
        self.security = security;
        let (_, port) = split_host(host);
        self.typed_by_hand = !port.is_empty();
        self.port = if self.typed_by_hand {
            port
        } else {
            standard_port(self.kind, security).to_string()
        };
    }

    /// The `host:port` this field submits, or the bare host when it carries no port of its own.
    pub(crate) fn dial(&self, host: &str) -> String {
        let (host, typed) = split_host(host);
        // A host the user already wrote a port into wins: two ports would be a contradiction, and
        // the one in the host field is the one they can see beside the name.
        let port = if typed.is_empty() {
            self.port.clone()
        } else {
            typed
        };
        server_address(host, port)
    }
}

/// Splits a typed server into host and port, as the core does: the core decides the address
/// actually dialled, so a form that split differently would show one port and connect to another.
pub(crate) fn split_host(input: &str) -> (String, String) {
    let parts = server_address_parts(input.to_owned());
    (parts.host, parts.port)
}

/// An account's two servers, so the manual form carries one field rather than four.
#[derive(Clone, Debug)]
pub(crate) struct ServerPair {
    pub(crate) imap: ServerField,
    pub(crate) smtp: ServerField,
}

impl Default for ServerPair {
    fn default() -> Self {
        Self {
            imap: ServerField::new(MailServerKind::Imap),
            smtp: ServerField::new(MailServerKind::Smtp),
        }
    }
}
