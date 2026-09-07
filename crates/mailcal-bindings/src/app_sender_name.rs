//! The sender-name FFI: the name an account's outgoing mail is sent under, the value to fill
//! its field in with, and whether a client may offer that field at all.
//!
//! Its own file rather than another entry in `app_settings.rs`, because the two reads below
//! are not settings reads. One of them talks to the provider, and a client has to know that
//! before it calls it from a main thread.
//!
//! The current name is **not** here: it rides on `AccountRow::name`, with every other account
//! label, so a switcher and a settings card cannot show different answers. What the name is
//! for, and which providers keep a copy of their own, is `docs/sending.md`.

use engine_api::AccountId;

use crate::MailcalApp;

#[uniffi::export]
impl MailcalApp {
    /// Sets the name `account`'s outgoing mail is sent under; empty clears it and the account
    /// sends as a bare address.
    ///
    /// The core sanitises what it is given, so pass the field's text through unchanged: a
    /// client that validated first would be a second opinion about what a `From` header may
    /// contain, and the two would drift. Persisted, then pushed to the provider where the
    /// account holder owns its copy; signals `Surface::Settings`.
    pub fn set_account_sender_name(&self, account: String, name: String) {
        self.runtime
            .block_on(self.app.set_account_sender_name(&account, &name));
    }

    /// The name to fill a "your name" field in with: the one already set, else the one the
    /// provider holds for this account. Empty when neither exists, which means *ask*.
    ///
    /// ⚠️ **Talks to the provider**, so call it off the main thread, exactly like
    /// [`MailcalApp::add_account`]. It is meant for the screen that offers the field, once,
    /// and never for drawing a label: a label reads `AccountRow::name`.
    ///
    /// A provider that is slow, unreachable, or missing the scope its settings API needs
    /// answers empty rather than failing: not knowing the name is the ordinary state, and it
    /// is one the user can resolve by typing.
    #[must_use]
    pub fn suggested_sender_name(&self, account: String) -> String {
        let Ok(id) = AccountId::try_from(account.as_str()) else {
            return String::new();
        };
        self.runtime.block_on(self.app.suggested_sender_name(&id))
    }

    /// Whether a client may offer to change `account`'s sender name.
    ///
    /// `false` only where a provider holds the name and the account holder cannot change it:
    /// on a Microsoft mailbox the display name comes from the organisation's directory, so an
    /// editor there offers an edit that cannot land. Show the name and leave out the editor.
    ///
    /// Ask this rather than deciding from the account's kind. Which providers can be written
    /// to is the engine's answer, and a client that hard-codes it will be wrong the first time
    /// a provider changes its mind.
    #[must_use]
    pub fn sender_name_editable(&self, account: String) -> bool {
        let Ok(id) = AccountId::try_from(account.as_str()) else {
            return false;
        };
        self.runtime.block_on(self.app.sender_name_editable(&id))
    }
}
