//! Setting up an account one of the person's other devices offered. When the pass runs, what it
//! found and how each account is shared are the core's (`MailcalApp::allodia_devices`,
//! `AccountPage::sharing`); this client draws them and starts neither.

use super::{AppInput, AppModel};

impl AppModel {
    /// Sets up an account one of the person's other devices offered, on the route its record
    /// names.
    ///
    /// The ordinary flow with the work already done, which is what syncing an account list is
    /// *for*: the record says which provider and which servers, so this opens the same card
    /// detection would have produced instead of spending a round trip re-learning it; and for a
    /// domain that publishes no autoconfig, re-learning it would have found less and dropped the
    /// person onto the manual form for an account another device set up without trouble.
    ///
    /// The password is still asked for here, because no password travels. A record that names no
    /// server routes to detection rather than to an empty form
    /// ([`mailcal_bindings::offered_setup`]). Settings stays open behind it: the setup
    /// window is a modal of its own.
    pub(super) fn set_up_offered_account(
        &mut self,
        offer: mailcal_bindings::AllodiaAccountOffer,
        sender: relm4::Sender<AppInput>,
    ) {
        if !self.can_open_account_setup() {
            return;
        }
        let email = offer.email.clone();
        // `required` is re-derived rather than assumed false: an offer accepted on the FIRST run
        // must leave the window required, or the screen the person cannot skip becomes one they
        // can close into an app with no accounts.
        self.setup.open_on(self.has_no_accounts(), email.clone());
        self.account_detected(email, mailcal_bindings::offered_setup(offer), sender);
    }
}
