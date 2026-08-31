//! The Microsoft (Graph mail + calendar) sign-in FFI method on [`MailcalApp`]: completing a
//! Microsoft OAuth sign-in. Split out of `app_accounts.rs` to keep each file under the 500-line
//! limit: the sibling of `app_accounts_google.rs`, which came out of the same file for the same
//! reason. The object itself is defined in `lib.rs`, and UniFFI collects these exported methods
//! crate-wide.

use std::sync::Arc;

use mailcal_account::GraphTokenSource;
use time::OffsetDateTime;

use crate::{AccountRow, ConnectedAccount, MailcalApp, MailcalError, connection_log, microsoft};

#[uniffi::export]
impl MailcalApp {
    /// Completes a Microsoft OAuth sign-in from the host's held `pending` handle (from
    /// [`begin_microsoft_login`](crate::begin_microsoft_login)) and the browser's redirect
    /// `callback_url`: validates the redirect, exchanges the code, discovers the account's
    /// address, connects its Graph mail folders, **writes the grant to the host's secure store**
    /// through [`crate::credential_store`], and joins it to the unified inbox. Returns the
    /// account row.
    ///
    /// The host does not store anything itself. It used to be handed the config TOML back for
    /// exactly that, which meant a freshly minted refresh token travelled out through view code
    /// and returned; three clients doing the same sequencing, none of them able to see a
    /// rotation the connect had already performed.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, or
    /// [`MailcalError::Connect`]/[`MailcalError::Engine`] if the token exchange, address
    /// lookup, folder connect, or the credential write fails. In every case the account is not
    /// added and any previously stored grant for it is untouched.
    pub fn complete_microsoft_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<AccountRow, MailcalError> {
        let started = std::time::Instant::now();
        log::info!("microsoft: completing sign-in; exchanging code + address lookup");
        let now = OffsetDateTime::now_utc();
        let authorized = match self.runtime.block_on(microsoft::authorize(
            &pending,
            &callback_url,
            now,
        )) {
            Ok(authorized) => authorized,
            Err(err) => {
                // A re-consent (or first connect) that failed to complete: the user declined, an
                // org policy blocked the app, or the exchange/`/me` lookup failed. Log it for
                // support so "I tapped Reconnect and nothing happened" is answerable: no account
                // was added, the existing grant is unchanged, and any raised re-consent prompt
                // stays up for another try. The error is the same OAuth protocol string already
                // shown on the sign-in surface (rule 9) and predates any token mint; no
                // credentials in it.
                log::warn!(
                    "oauth: Microsoft sign-in did not complete ({err}); grant unchanged, any re-consent prompt remains"
                );
                return Err(err);
            }
        };
        let config = authorized.config;
        log::info!(
            "microsoft: token exchange + address lookup ok in {}ms",
            started.elapsed().as_millis(),
        );
        let account_id = config
            .account_id()
            .map_err(|err| MailcalError::Engine(err.to_string()))?;
        let row = AccountRow {
            id: account_id.as_str().to_owned(),
            email: config.email.clone(),
            // A just-added account opens showing its folders (the persisted default).
            expanded: true,
        };
        // Build the shared token source, seeded with the just-minted access token so the first sync
        // needs no immediate refresh, and REGISTER before dialing: so a rotation during the dial
        // has an entry to land in. The same `tokens` goes into the entry the dial
        // reads back, so there is one refresher of this credential and not two. Nothing
        // re-inserts the config afterwards: the sink may have advanced it, and writing the
        // parsed-from-TOML copy back over that would restore the superseded token.
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let tokens = GraphTokenSource::new(
            &config,
            account_id.clone(),
            Some(sink),
            mailcal_account::CredentialOrigin::FreshSignIn,
        )
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
        tokens.seed_access_token(authorized.access_token, authorized.expires_at);
        let registered = self.registry.pre_register(
            account_id.as_str().to_owned(),
            ConnectedAccount::Microsoft {
                config,
                tokens: Arc::clone(&tokens),
            },
        );
        log::info!("microsoft: connecting Graph mail folders and calendar");
        // One dial, the same one boot and reconnect use; obtainable only from the registry, which
        // is what makes the ordering above unskippable rather than merely documented.
        let Some(dial) = self.registry.dial(account_id.as_str()) else {
            registered.rollback(&self.registry);
            return Err(MailcalError::Config(
                "the account could not be registered before connecting".to_owned(),
            ));
        };
        let outcome = match self
            .runtime
            .block_on(dial.run(&account_id, self.device_zone.clone()))
        {
            Ok(outcome) => outcome,
            Err(err) => {
                // The sign-in produced a grant but the account never came up. This is also the
                // re-auth path, so put back whatever entry was there rather than dropping a live
                // account's re-connection state.
                registered.rollback(&self.registry);
                return Err(MailcalError::Connect(err.to_string()));
            }
        };
        // The grant works, so store it; from the registry, not from the config parsed a moment
        // ago, because the dial above may already have rotated the refresh token through
        // the sink. A refused write puts the registry back: on a re-auth that restores the
        // previous grant, which is still what the store holds and still what the next
        // launch will load.
        if let Err(err) = self.persist_registered_grant(account_id.as_str()) {
            registered.rollback(&self.registry);
            return Err(self.abandon_unstorable_account(
                account_id.as_str(),
                mailcal_app::Protocol::Graph,
                &err,
            ));
        }
        let calendar_reauth_required = outcome.calendar_reauth_required;
        let calendar_providers_len = outcome.account.calendar_providers.len();
        let calendar_connected = calendar_providers_len > 0;
        log::info!(
            "microsoft: connected {} folder provider(s) and {calendar_providers_len} calendar \
             provider(s); syncing + adding account",
            outcome.account.providers.len(),
        );
        self.refresh_analytics_accounts();
        let sync_id = account_id.clone();
        let reauth_id = account_id;
        let account = outcome.account;
        connection_log::log_account_connection_info("new-account", "graph", &account);
        // Register the account **without** syncing, so it appears in the switcher and the setup
        // modal can dismiss at once. The account gets the product default three-month depth before
        // the visible first sync starts in the background.
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.add_new_account_deferred(account).await });
        // Reconcile the calendar re-consent prompt: a successful calendar connect clears it (this
        // is the path a re-auth completes through), a scope-`403` raises it.
        if calendar_connected {
            self.app.clear_calendar_reauth_required(&reauth_id);
        } else if calendar_reauth_required {
            self.app.note_calendar_reauth_required(&reauth_id);
        }
        // A completed sign-in re-grants the whole scope set, so the mail write/send permission is
        // restored; clear any standing "reconnect to send and manage mail" prompt (one re-consent
        // covers both this and the calendar prompt above). Optimistic: if the fresh grant still
        // lacked `Mail.Send` (e.g. an admin restriction), the next refused send re-raises it.
        self.app.clear_mail_reauth_required(&reauth_id);
        log::info!(
            "oauth: Microsoft sign-in complete; cleared any mail re-consent prompt (the new grant \
             requests mail read/write + send); calendar {}",
            if calendar_connected {
                "connected"
            } else {
                "unavailable (mail-only grant)"
            },
        );
        self.refresh_background(&row.id);
        let app_sync = Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app_sync.sync_added_account(&sync_id).await });
        log::info!(
            "microsoft: account registered in {}ms; first sync running in the background",
            started.elapsed().as_millis(),
        );
        Ok(row)
    }
}
