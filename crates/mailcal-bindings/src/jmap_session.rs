//! What a JMAP account's session says it offers, read at every dial (RFC 8620 §2: the signed-in
//! account's `accountCapabilities`) and kept with the account, so Settings and setup can tell a
//! use the server has none of from one the person switched off.

use std::sync::{Arc, Mutex};

use mailcal_account::{Capabilities, Capability};

/// The uses of mail, calendar and contacts a JMAP account's session did not offer at its last
/// dial. Shared between the registry entry and every dial taken from it, as a token source is, so
/// the dial records what it read without holding the registry. Empty until a dial has read the
/// session.
#[derive(Debug, Clone, Default)]
pub(crate) struct JmapSession(Arc<Mutex<Capabilities>>);

impl JmapSession {
    /// Records what a session just read offers.
    pub(crate) fn record(&self, offered: engine_api::Capabilities) {
        *self.0.lock().expect("jmap session mutex poisoned") = lacks(offered);
    }

    /// The uses the session did not offer at the last dial.
    pub(crate) fn lacks(&self) -> Capabilities {
        self.0.lock().expect("jmap session mutex poisoned").clone()
    }
}

/// The uses of mail, calendar and contacts `offered` has none of.
fn lacks(offered: engine_api::Capabilities) -> Capabilities {
    [
        (Capability::Mail, offered.mail()),
        (Capability::Calendar, offered.calendars()),
        (Capability::Contacts, offered.contacts()),
    ]
    .into_iter()
    .filter_map(|(capability, has)| (!has).then_some(capability))
    .collect()
}

#[cfg(test)]
#[path = "jmap_session_tests.rs"]
mod tests;
