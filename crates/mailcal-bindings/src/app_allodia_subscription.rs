// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The account-screen FFI methods on [`MailcalApp`]. The records they carry are in
//! [`crate::allodia_subscription`], and the rules they apply are `purchasing.md`'s, the contract
//! beside the Allodia Licence.
//!
//! Five calls: one read that answers the whole screen, and four writes that only ever touch
//! **Allodia's own** subscription. A store's is changed at that store, and the read hands back the
//! page to open.
//!
//! **All five block.** Each makes a token round trip and a request, so a host calls them off the
//! main thread, exactly as it already does for `sync_allodia_accounts`.

use crate::{
    MailcalApp,
    allodia_purchase::{AllodiaPlan, AllodiaPurchaseError},
    allodia_subscription::{
        AllodiaCancellation, AllodiaCheckout, AllodiaIntervalChange, AllodiaSubscription,
    },
};

#[uniffi::export]
impl MailcalApp {
    /// Everything the Allodia account screen needs, in one read.
    ///
    /// ⚠️ **Not what gates a capability.** This is what the screen *says*; what to switch on comes
    /// from the entitlement, which is cached and never blocks. Reading this to decide whether a
    /// feature is on would put a network call in front of it.
    ///
    /// **Blocking.** Call it off the main thread.
    ///
    /// # Errors
    ///
    /// [`AllodiaPurchaseError::Unavailable`] in a build with no Allodia registration,
    /// [`AllodiaPurchaseError::NotSignedIn`] when nobody is signed in,
    /// [`AllodiaPurchaseError::NeedsReauth`] when the stored sign-in predates the permission, and
    /// [`AllodiaPurchaseError::Unreachable`] when the service could not be asked.
    pub fn allodia_subscription(&self) -> Result<AllodiaSubscription, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.read_allodia_subscription()
        }
    }

    /// Get a payment page for a new subscription, to open in a **browser**.
    ///
    /// For the builds that sell through Allodia: Windows, Linux and the direct macOS download. On
    /// iOS and Android the store's own purchase is required, and `link_allodia_purchases` attaches
    /// the result.
    ///
    /// A client checks `actions.can_start_checkout` first; this refuses otherwise, which is what
    /// stops it selling a second subscription to somebody a store is already charging.
    ///
    /// **Blocking.** Call it off the main thread.
    ///
    /// # Errors
    ///
    /// As [`MailcalApp::allodia_subscription`], plus
    /// [`AllodiaPurchaseError::Refused`] carrying why.
    pub fn start_allodia_checkout(
        &self,
        plan: AllodiaPlan,
    ) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            let _ = plan;
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.run_allodia_checkout(plan)
        }
    }

    /// Stop the recurring charge, keeping everything until the period already paid for runs out.
    ///
    /// ⚠️ Only Allodia's own subscription. One bought through a store is cancelled at that store,
    /// and its `manage_url` is where to send the person.
    ///
    /// **Blocking.** Call it off the main thread.
    ///
    /// # Errors
    ///
    /// As [`MailcalApp::allodia_subscription`], plus
    /// [`AllodiaPurchaseError::Refused`] carrying why.
    pub fn cancel_allodia_subscription(&self) -> Result<AllodiaCancellation, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.run_allodia_cancel()
        }
    }

    /// Move between the monthly and the yearly plan.
    ///
    /// ⚠️ **Changes the next charge and nothing else.** No money moves today, nothing is refunded,
    /// and the period already paid for runs on untouched. Say so before asking anybody to confirm:
    /// the switch is otherwise silent until a date that may be a month away.
    ///
    /// **Blocking.** Call it off the main thread.
    ///
    /// # Errors
    ///
    /// As [`MailcalApp::allodia_subscription`], plus
    /// [`AllodiaPurchaseError::Refused`] carrying why.
    pub fn switch_allodia_interval(
        &self,
        plan: AllodiaPlan,
    ) -> Result<AllodiaIntervalChange, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            let _ = plan;
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.run_allodia_switch(plan)
        }
    }

    /// Start a cancelled subscription again.
    ///
    /// Comes back either restarted on the authorisation already held, with nothing to pay and
    /// nothing to re-enter, or with a page to open. A client draws both.
    ///
    /// **Blocking.** Call it off the main thread.
    ///
    /// # Errors
    ///
    /// As [`MailcalApp::allodia_subscription`], plus
    /// [`AllodiaPurchaseError::Refused`] carrying why.
    pub fn resubscribe_to_allodia(&self) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.run_allodia_resubscribe()
        }
    }
}

#[cfg(feature = "allodia-license")]
mod calls {
    use allodia_license::{AccountService, Error, Feature};

    use super::{
        AllodiaCancellation, AllodiaCheckout, AllodiaIntervalChange, AllodiaPlan,
        AllodiaPurchaseError, AllodiaSubscription,
    };
    use crate::{AllodiaGrantHealth, MailcalApp, allodia_transport::HttpsTransport};

    /// Everything a call needs, gathered once so each of the five reads down the page.
    struct Ready {
        service: AccountService,
        transport: HttpsTransport,
        token: String,
    }

    impl MailcalApp {
        /// Mint a token, check the grant permits `feature`, and build the transport.
        ///
        /// The order matters and is the same as the sync pass's: the token first, because minting
        /// one is what learns the grant's scopes on an install that never recorded them, and the
        /// scope check before the first request, because that request cannot succeed and its
        /// refusal would read as the service being down.
        fn ready(&self, feature: Feature) -> Result<Ready, AllodiaPurchaseError> {
            let token = self.allodia_access_token().map_err(|_| {
                if self.allodia.lock().expect("allodia account lock").is_none() {
                    AllodiaPurchaseError::NotSignedIn
                } else {
                    AllodiaPurchaseError::Unreachable
                }
            })?;
            if !self.allodia_grant_permits(feature) {
                self.note_allodia_health(AllodiaGrantHealth::NeedsReauth);
                return Err(AllodiaPurchaseError::NeedsReauth);
            }
            let transport = HttpsTransport::new(self.runtime.handle().clone())
                .map_err(|_| AllodiaPurchaseError::Unreachable)?;
            Ok(Ready {
                service: AccountService::new(allodia_license::host()),
                transport,
                token,
            })
        }

        pub(super) fn read_allodia_subscription(
            &self,
        ) -> Result<AllodiaSubscription, AllodiaPurchaseError> {
            let ready = self.ready(Feature::ReadSubscription)?;
            let subscription = ready
                .service
                .subscription(&ready.transport, &ready.token)
                .map_err(|error| self.classify(&error))?;
            self.note_allodia_health(AllodiaGrantHealth::Ok);
            Ok(subscription.into())
        }

        pub(super) fn run_allodia_checkout(
            &self,
            plan: AllodiaPlan,
        ) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .start_checkout(&ready.transport, &ready.token, plan.into())
                .map(Into::into)
                .map_err(|error| self.classify(&error))
        }

        pub(super) fn run_allodia_cancel(
            &self,
        ) -> Result<AllodiaCancellation, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .cancel_subscription(&ready.transport, &ready.token)
                .map(Into::into)
                .map_err(|error| self.classify(&error))
        }

        pub(super) fn run_allodia_switch(
            &self,
            plan: AllodiaPlan,
        ) -> Result<AllodiaIntervalChange, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .switch_interval(&ready.transport, &ready.token, plan.into())
                .map(Into::into)
                .map_err(|error| self.classify(&error))
        }

        pub(super) fn run_allodia_resubscribe(
            &self,
        ) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .resubscribe(&ready.transport, &ready.token)
                .map(Into::into)
                .map_err(|error| self.classify(&error))
        }

        /// Turn one failure into what a client draws, and record what it says about the grant.
        ///
        /// **A refusal and an outage are different answers**, the distinction the entitlement
        /// contract is built on. A service that declined to act told us something about the
        /// subscription; a service that could not be reached told us nothing, and in particular
        /// nothing about the grant.
        fn classify(&self, error: &Error) -> AllodiaPurchaseError {
            match error {
                Error::Unauthorized => {
                    self.note_allodia_health(AllodiaGrantHealth::SignedOut);
                    AllodiaPurchaseError::Unreachable
                }
                Error::Refused(refusal) => {
                    self.note_allodia_health(AllodiaGrantHealth::Ok);
                    AllodiaPurchaseError::Refused {
                        reason: refusal.clone().into(),
                    }
                }
                _ => AllodiaPurchaseError::Unreachable,
            }
        }
    }
}

#[cfg(test)]
#[path = "app_allodia_subscription_tests.rs"]
mod tests;
