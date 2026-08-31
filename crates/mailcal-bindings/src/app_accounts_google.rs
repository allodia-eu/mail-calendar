//! The Google (Gmail + Google Calendar) sign-in FFI method on [`MailcalApp`]: completing a
//! Google OAuth sign-in. Split out of `app_accounts.rs` to keep each file under the 500-line
//! limit; the object itself is defined in `lib.rs`, and UniFFI collects these exported methods
//! crate-wide.

use std::sync::Arc;

use time::OffsetDateTime;

use crate::{AccountRow, ConnectedAccount, MailcalApp, MailcalError, connection_log, google};

#[uniffi::export]
impl MailcalApp {
    /// Completes a Google OAuth sign-in from the host's held `pending` handle (from
    /// [`begin_google_login`](crate::begin_google_login)) and the browser's redirect
    /// `callback_url`: validates the redirect, exchanges the code, discovers the account's
    /// address (Gmail profile), connects its Gmail provider + Google Calendar, **writes the grant
    /// to the host's secure store** through [`crate::credential_store`], and joins it to the
    /// unified inbox. Returns the account row; the host stores nothing itself.
    ///
    /// The client is responsible for the Early Access gate (showing the notice and confirming
    /// sign-up) *before* it starts this flow; the core is unaware of it. If Google does not yet
    /// have the user's address as an allow-listed test user, its consent screen blocks the
    /// sign-in and this returns [`MailcalError::Connect`]; surfaced on the setup surface.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed, or
    /// [`MailcalError::Connect`]/[`MailcalError::Engine`] if the token exchange, address lookup,
    /// mail connect, or the credential write fails. In every case the account is not added and
    /// any previously stored grant for it is untouched.
    pub fn complete_google_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<AccountRow, MailcalError> {
        let started = std::time::Instant::now();
        log::info!("google: completing sign-in; exchanging code + address lookup");
        let now = OffsetDateTime::now_utc();
        let authorized = self
            .runtime
            .block_on(google::authorize(&pending, &callback_url, now))
            .map_err(|err| {
                log::warn!("google: sign-in failed at token exchange / address lookup: {err}");
                err
            })?;
        let config = authorized.config;
        log::info!(
            "google: token exchange + address lookup ok in {}ms",
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
        // parsed copy back over that would restore the superseded token. Google re-issues a
        // refresh token seldom, which is exactly why this path must not depend on noticing
        // when it does.
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        let tokens = mailcal_account::google_token_source(
            &config,
            account_id.clone(),
            Some(sink),
            mailcal_account::CredentialOrigin::FreshSignIn,
        )
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
        tokens.seed_access_token(authorized.access_token, authorized.expires_at);
        let registered = self.registry.pre_register(
            account_id.as_str().to_owned(),
            ConnectedAccount::Google {
                config,
                tokens: Arc::clone(&tokens),
            },
        );
        log::info!("google: connecting the Gmail provider and calendar");
        // One dial, the same one boot and reconnect use; obtainable only from the registry.
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
                log::warn!("google: Gmail provider connect failed: {err}");
                // Put back whatever entry was there (a re-sign-in of an existing account) rather
                // than dropping a live account's re-connection state.
                registered.rollback(&self.registry);
                return Err(MailcalError::Connect(err.to_string()));
            }
        };
        // The grant works, so store it; from the registry rather than from the config parsed a
        // moment ago. Google re-issues a refresh token seldom, which is exactly why this
        // must not depend on noticing when it does.
        if let Err(err) = self.persist_registered_grant(account_id.as_str()) {
            registered.rollback(&self.registry);
            return Err(self.abandon_unstorable_account(
                account_id.as_str(),
                mailcal_app::Protocol::Google,
                &err,
            ));
        }
        log::info!(
            "google: connected mail with {} calendar provider(s); syncing + adding account",
            outcome.account.calendar_providers.len(),
        );
        self.refresh_analytics_accounts();
        let sync_id = account_id;
        let account = outcome.account;
        connection_log::log_account_connection_info("new-account", "google", &account);
        // Register the account **without** syncing, so it appears in the switcher and the setup
        // modal can dismiss at once, with the product default depth recorded before the visible
        // first sync starts in the background.
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.add_new_account_deferred(account).await });
        self.refresh_background(&row.id);
        let app_sync = Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app_sync.sync_added_account(&sync_id).await });
        log::info!(
            "google: account registered in {}ms; first sync running in the background",
            started.elapsed().as_millis(),
        );
        Ok(row)
    }
}
