//! Writing an account's credential to the host's OS secure store, and erasing it again.
//!
//! Split from the registry itself because the two answer different questions: the registry holds
//! what this process knows, these four write what the *next* launch will know, and because both
//! halves grew past what one file under the 500-line cap can hold.
//!
//! Two rules live here rather than at the call sites. **The bytes come from the registry, not from
//! a caller's string** ([`MailcalApp::persist_registered_grant`]): a connect refreshes, a refresh
//! can rotate, and a caller's TOML was serialized before either happened. And **every write is
//! logged here**, at the one place they all funnel through, because a success used to leave no line
//! at all: so a support log showed a flawless sign-in and said nothing about whether the
//! credential ever reached the device.

use crate::{
    MailcalApp, MailcalError,
    credential_log::{self, CredentialOp},
};

impl MailcalApp {
    /// Writes `config_toml` into the host's secure store as `id`'s credential.
    ///
    /// For a config the core never rewrites: an IMAP account, which has no grant and so nothing
    /// that can rotate. Anything with a grant goes through [`Self::persist_registered_grant`],
    /// whose whole job is to not write a stale copy.
    ///
    /// Both outcomes are logged **here**, at the one place every credential write funnels
    /// through, rather than left to the callers. Two reasons, and the second is the one that
    /// caught us. A *successful* write had no line at all: the step the core took over from the
    /// hosts was the only step in an add that left no trace, so a support log showed a flawless
    /// sign-in and said nothing about whether the credential reached the device: the same
    /// unfalsifiable shape this port's `Result` exists to remove, one level up. And a *failed*
    /// write was logged by two of its three callers: the re-auth path
    /// (`jmap_oauth::reauth`) propagates the error with `?` and logged nothing, so a re-auth
    /// whose store write was refused was silent end to end. A caller that adds consequence still
    /// should ([`Self::abandon_unstorable_account`] says what happens to the account) but no
    /// caller has to remember to report the write itself.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] if the host's store refused the write.
    pub(crate) fn persist_credential(
        &self,
        id: &str,
        config_toml: String,
    ) -> Result<(), MailcalError> {
        match self.credential_store.persist(id.to_owned(), config_toml) {
            Ok(()) => {
                log::info!("{}", credential_log::ok_line(CredentialOp::Store, id));
                Ok(())
            }
            Err(err) => {
                log::error!(
                    "{}",
                    credential_log::refused_line(CredentialOp::Store, id, &err.to_string()),
                );
                Err(MailcalError::Connect(err.to_string()))
            }
        }
    }

    /// Writes the registry's **current** config for the OAuth account `id` into the host's
    /// secure store.
    ///
    /// Serializing from the registry rather than from a caller's string is what makes this safe
    /// to call after a connect: a rotation during that connect has already advanced the registry,
    /// and the caller's TOML still carries the token it replaced. See this module's header.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `id` is not a registered OAuth account or its config
    /// cannot be serialized, and [`MailcalError::Connect`] if the host's store refused the write.
    /// Whether that is recoverable is the caller's to decide; it depends entirely on what the
    /// caller was in the middle of.
    pub(crate) fn persist_registered_grant(&self, id: &str) -> Result<(), MailcalError> {
        let config_toml = self
            .registry
            .oauth_config_toml(id)
            .map_err(MailcalError::Config)?;
        self.persist_credential(id, config_toml)
    }

    /// Erases `id`'s entry from the host's secure store, so the account does not return at the
    /// next launch.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] if the host's store refused the delete. The account is
    /// already gone from the runtime by then; what survives is the stored credential, which comes
    /// back as an account at the next launch.
    pub(crate) fn delete_stored_credential(&self, id: &str) -> Result<(), MailcalError> {
        match self.credential_store.delete(id.to_owned()) {
            Ok(()) => {
                log::info!("{}", credential_log::ok_line(CredentialOp::Erase, id));
                Ok(())
            }
            // The caller (`remove_account`) says what this *means* for the account; this says
            // what happened, so the two read as one story and neither depends on the other
            // being written.
            Err(err) => {
                log::error!(
                    "{}",
                    credential_log::refused_line(CredentialOp::Erase, id, &err.to_string()),
                );
                Err(MailcalError::Connect(err.to_string()))
            }
        }
    }

    /// Gives up on an account that connected but whose credential the host's store refused,
    /// after the caller has already put the registry back.
    ///
    /// The account is dropped rather than kept, and that is the deliberate half. Keeping it would
    /// hand the user a working mailbox that is simply *gone* after the next launch, with no
    /// moment at which anything was reported; whereas failing here fails on the setup surface
    /// the user is still looking at, where a retry is one tap away. For an OAuth account the
    /// grant stays minted on the server and is orphaned, which costs nothing: signing in again
    /// mints another.
    pub(crate) fn abandon_unstorable_account(
        &self,
        id: &str,
        protocol: mailcal_app::Protocol,
        err: &MailcalError,
    ) -> MailcalError {
        log::error!(
            "credentials: [{}] the account connected, but the host's store refused its credential \
             ({err}): the account is being dropped rather than left to disappear at the next \
             launch",
            mailcal_account::account_log_handle(id),
        );
        self.app.track(mailcal_app::Event::SetupFailed { protocol });
        MailcalError::Connect(format!(
            "the account connected, but its credential could not be saved to this device's \
             secure store ({err})",
        ))
    }
}
