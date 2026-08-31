//! The [`MailcalApp`] constructors and account-lifecycle FFI methods: building demo /
//! showcase / real-account apps, and adding and removing accounts. Split out of `lib.rs` to keep
//! each file under the 500-line limit; the object itself is defined in `lib.rs`, and UniFFI
//! collects these exported methods crate-wide. The two browser sign-in completions live in the
//! siblings `app_accounts_microsoft.rs` and `app_accounts_google.rs`; each came out of this file
//! for the same reason, and each is one self-contained provider flow.

use std::sync::Arc;

use engine_api::AccountId;

use crate::{
    AccountCredentialStore, AccountRow, DeviceInfo, LogLevel, Logger, MailcalApp, MailcalError,
    Observer, ShowcaseLocale, boot, connection_log,
};

#[uniffi::export]
impl MailcalApp {
    /// Builds an in-memory demo app: an ephemeral engine seeded by a
    /// demo provider's sample mail, notifying `observer` on changes. `device_timezone`
    /// is the host's current OS zone (an IANA id); the demo does not persist it.
    ///
    /// `logger` is the host's logging sink (every layer's diagnostics route to it) and
    /// `log_level` its initial ceiling; see [`MailcalApp::new_accounts`].
    #[uniffi::constructor]
    pub fn new_demo(
        observer: Box<dyn Observer>,
        logger: Box<dyn Logger>,
        log_level: LogLevel,
        device_timezone: String,
    ) -> Arc<Self> {
        boot::build_demo(observer, logger, log_level, device_timezone)
    }

    /// Builds an in-memory **showcase** app for taking store/marketing screenshots: two
    /// fictional accounts (a full mailbox with folders, a threaded conversation, an
    /// attachment, and a calendar; plus a lighter second account, so the unified inbox and
    /// switcher look real) seeded entirely from bundled sample content: no real account, no
    /// network, so no personal mail can leak into a screenshot. A host opts into this with the
    /// `MAILCAL_SHOWCASE` launch flag (an env var on macOS/iOS/Windows, an intent extra on
    /// Android); it is never used in a shipped build.
    ///
    /// `locale` picks the language of that sample content; its mail, folder names, and
    /// calendar: so the host can seed it to match the UI language it renders and take a
    /// coherent screenshot per store listing.
    ///
    /// Distinct from [`MailcalApp::new_demo`], the tiny fixture the CI verify gates assert on;
    /// enriching that would break them, so the screenshot dataset lives on its own. `logger`,
    /// `log_level`, and `device_timezone` are as in [`MailcalApp::new_accounts`], nothing is
    /// persisted.
    #[uniffi::constructor]
    pub fn new_showcase(
        observer: Box<dyn Observer>,
        logger: Box<dyn Logger>,
        log_level: LogLevel,
        device_timezone: String,
        locale: ShowcaseLocale,
    ) -> Arc<Self> {
        boot::build_showcase(observer, logger, log_level, device_timezone, locale)
    }

    /// Builds a real account-backed app from the host's stored account `configs` (each a
    /// TOML blob of endpoints + credentials, read from the OS secure store; Keychain /
    /// EncryptedSharedPreferences: not a plaintext file): opens one on-disk engine shared
    /// by every account, connects each account's IMAP folders over a certificate-verifying
    /// TLS connector, and notifies `observer` on changes. Each connect blocks on the
    /// internal runtime, so this returns only once every account has been attempted. An
    /// empty `configs` brings up an account-less app (first run, before the user adds one);
    /// the host then calls [`MailcalApp::add_account`].
    ///
    /// One account that fails to connect (a stale password, a server blip) does **not**
    /// block the others: the app comes up with whatever connected, and each skipped account
    /// is recorded in [`MailcalApp::account_connect_error`] for the host to surface. A
    /// configured calendar that fails to connect (mail still up) is recorded separately in
    /// [`MailcalApp::calendar_connect_error`]. (Adding an account interactively *does*
    /// surface its mail error, see [`MailcalApp::add_account`].)
    ///
    /// Each account's id is derived from its login username **and host** (both lowercased,
    /// see [`mailcal_account::AccountConfig::account_id`]), so the same mailbox keeps a
    /// stable identity in the shared engine store across launches while the same username
    /// on different servers stays distinct.
    ///
    /// `data_dir` is the host's writable app-data directory: the on-disk engine store
    /// (`mailcal.sqlite`) and the display-zone preference (`preferences.toml`) are
    /// created under it. The host owns this path; macOS passes its application-support
    /// location, Android passes its `filesDir`; because there is no portable `$HOME`.
    ///
    /// `device_timezone` is the host's current OS zone (an IANA id): on first boot it is
    /// adopted and persisted as the display zone; on a later launch in a different zone
    /// the stored zone stays active and the device zone is offered as a pending change.
    ///
    /// `credential_store` is the host's OS-secure-store writer, and it is a **parameter rather
    /// than a setter** because this constructor starts dialing before it returns: its last
    /// statement kicks off the background connect, and on a production device the first OAuth
    /// refresh began 6 ms later; with the host still blocked here, unable to install anything.
    /// Two clients installed their store from a UI-thread post, so whether a rotation arriving
    /// half a second later was saved or lost came down to whether the main thread got a turn
    /// first. See [`crate::credential_store`].
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError`] only for failures that prevent *any* account from working:
    /// the runtime cannot start or the engine cannot open. A single account's
    /// load/connect failure is non-fatal (recorded, not returned).
    #[allow(clippy::too_many_arguments)]
    #[uniffi::constructor]
    pub fn new_accounts(
        observer: Box<dyn Observer>,
        logger: Box<dyn Logger>,
        log_level: LogLevel,
        configs: Vec<String>,
        data_dir: String,
        device_timezone: String,
        device_info: DeviceInfo,
        credential_store: Box<dyn AccountCredentialStore>,
    ) -> Result<Arc<Self>, MailcalError> {
        boot::build_accounts(
            boot::HostPorts {
                observer,
                logger,
                credential_store: Arc::from(credential_store),
            },
            log_level,
            configs,
            data_dir,
            boot::HostDevice {
                timezone: device_timezone,
                info: device_info,
            },
            boot::BootMode {
                // The interactive app runs the live IMAP IDLE watches / poll timers.
                start_live_sync: true,
            },
        )
    }

    /// Connects an additional account from its stored `config_toml` at runtime and joins it to
    /// the unified inbox, then drives its (possibly large) first sync in the **background** so
    /// the setup modal dismisses at once instead of blocking on the download: the same
    /// deferred shape the Microsoft path uses. The connect itself stays synchronous so a bad
    /// password / unreachable server is surfaced on the form. The host stores the config in its
    /// secure store and calls this after the user completes the account-setup form. Returns the
    /// new account's [`AccountRow`] (id + email) so the host can key the stored config and
    /// update its switcher.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError`] if the config cannot be loaded or its IMAP connection/login
    /// fails. A calendar-connect failure is non-fatal; recorded in
    /// [`MailcalApp::calendar_connect_error`] rather than returned.
    pub fn add_account(&self, config_toml: String) -> Result<AccountRow, MailcalError> {
        // Parse, derive the id, build the account's ONE token source, and REGISTER it; all before
        // a socket is opened, and through exactly the code both boot modes use. The kind is
        // detected by parsing rather than passed in, so there is no `is_jmap` flag left to
        // get wrong.
        let sink = crate::token_sink::token_sink(&self.registry, &self.credential_store);
        // A credential the host has only now handed us replaces anything this process remembers
        // about the account; re-adding one that was removed must not inherit its dead state.
        let prepared = boot::prepare_stored_account(
            &config_toml,
            &sink,
            mailcal_account::CredentialOrigin::FreshSignIn,
        )?;
        let account_id = prepared.account.id.clone();
        let row = AccountRow {
            id: account_id.as_str().to_owned(),
            email: prepared.account.identity.email.clone(),
            // A just-added account opens showing its folders: the persisted default, and the
            // only sensible one for a mailbox the user has this second asked for.
            expanded: true,
        };
        let protocol = prepared.connected.protocol();
        let family = prepared.connected.account_type();
        // The setup funnel: which protocol was attempted, and did it connect. Paired with
        // `setup_completed` / `setup_failed` below, this answers "how many people never get an
        // account connected, and on which protocol": the churn that happens before first use.
        // No-ops unless the user has consented.
        self.app
            .track(mailcal_app::Event::SetupStarted { protocol });
        // The narration this path had none of, while `complete_microsoft_login` next door has
        // carried seven lines for as long as it has existed. An IMAP/JMAP add appeared in a support
        // log only as raw `connect[…]` steps: so an add that *hung* (a wrong host, a firewall, a
        // server that never answers) produced a log in which nothing had happened, and there was no
        // line to say an add had even been attempted, let alone over which protocol. The address
        // and the host stay out of it, as everywhere else.
        let started = std::time::Instant::now();
        log::info!("add-account: connecting a new {family} account");

        // Registered before the dial, holding whatever it displaced so re-adding an existing
        // account cannot lose it. The ordering is the whole point: the dial refreshes, the
        // refresh can rotate, and a rotation with no entry to land in is a credential the
        // next launch will not have.
        let registered = self
            .registry
            .pre_register(row.id.clone(), prepared.connected);
        let Some(dial) = self.registry.dial(&row.id) else {
            registered.rollback(&self.registry);
            return Err(MailcalError::Config(
                "the account could not be registered before connecting".to_owned(),
            ));
        };
        let zone = self.device_zone.clone();
        let outcome = match self.runtime.block_on(dial.run(&account_id, zone)) {
            Ok(outcome) => outcome,
            Err(err) => {
                // Put the registry back as it was: remove the entry this added, or restore the one
                // it displaced.
                registered.rollback(&self.registry);
                // Counted, not classified: the error's *string* is what carries the host and the
                // username, and it is not ours to send. See `mailcal_app::Event::SetupFailed`.
                self.app.track(mailcal_app::Event::SetupFailed { protocol });
                // Logged, though: an add that fails and says nothing is the case a support log most
                // needs to explain. It says the account was not added and stops there; it used to
                // add "and nothing was stored", which is not always true: if the sign-in's very
                // first refresh rotated the token before the mail host refused, the sink has
                // already written that grant to the store, and losing it would be
                // worse than keeping it.
                log::warn!(
                    "add-account: the {family} connect failed after {}ms ({err}): the account was \
                     not added",
                    started.elapsed().as_millis(),
                );
                return Err(MailcalError::Connect(err.to_string()));
            }
        };
        let account = outcome.account;
        let calendar_error = outcome.calendar_error;
        // Write the credential. For anything with a grant the bytes come from the **registry**, not
        // from `config_toml`: a rotation during the dial has already advanced the registered
        // config, and the caller's string still carries the token it replaced. An IMAP
        // account has no grant, so its stored config is exactly what the host passed in.
        let persisted = if matches!(protocol, mailcal_app::Protocol::Imap) {
            self.persist_credential(&row.id, config_toml)
        } else {
            self.persist_registered_grant(&row.id)
        };
        if let Err(err) = persisted {
            registered.rollback(&self.registry);
            return Err(self.abandon_unstorable_account(&row.id, protocol, &err));
        }
        // Nothing is re-inserted here: the registered entry already carries the live token source,
        // including any rotation the dial reported into it.
        registered.commit();
        // Connected *and* stored: this account will still be here at the next launch, which is
        // what the setup funnel is counting.
        self.app
            .track(mailcal_app::Event::SetupCompleted { protocol });
        self.refresh_analytics_accounts();
        if let Some(error) = calendar_error {
            self.calendar_connect_errors
                .lock()
                .expect("calendar-errors mutex poisoned")
                .push(error);
        }
        connection_log::log_account_connection_info("new-account", family, &account);
        // Register the connected account WITHOUT syncing (`add_new_account_deferred`) so it appears
        // in the switcher and the setup modal can dismiss at once, with the product default
        // three-month depth recorded before the first sync starts.
        let sync_id = account.id.clone();
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.add_new_account_deferred(account).await });
        // Start this account's background sync (push watches / poll timer) per its settings.
        self.refresh_background(&row.id);
        // The first sync runs with the download bar **visible**; adding an account is an explicit
        // download the user is waiting on, so it shows progress immediately.
        let app_sync = Arc::clone(&self.app);
        self.runtime
            .spawn(async move { app_sync.sync_added_account(&sync_id).await });
        // Closes the narration the way the Microsoft path does: connected, stored, registered,
        // and how long the whole transaction took: so the next line in the log being a sync is
        // an expected continuation rather than the first evidence that anything worked.
        log::info!(
            "add-account: [{}] {family} account connected, stored and registered in {}ms; first \
             sync running in the background",
            mailcal_account::account_log_handle(&row.id),
            started.elapsed().as_millis(),
        );
        Ok(row)
    }

    /// Removes the account `id`: stops its background sync, drops it from the reconnection
    /// registry, removes it from the runtime (switcher, selection) so its mail leaves the list,
    /// and **erases its credential** from the host's OS secure store so it does not return at the
    /// next launch. The observer fires as the snapshot rebuilds. A no-op for an unknown id.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] if the host's store refused to erase the credential.
    /// The account is gone from the runtime regardless (that half cannot fail) so this is
    /// reported rather than rolled back: what survives is a stored credential with no account,
    /// which comes back as an account at the next launch with nothing to explain it. A host
    /// should surface it; the removal itself has happened.
    pub fn remove_account(&self, id: String) -> Result<(), MailcalError> {
        // First, and before the config it was derived from goes: the person's other devices have
        // to learn this account went, or it comes back to them as an offer. Best effort; a
        // removal the person has already made is not undone by a service that did not answer.
        #[cfg(feature = "allodia-license")]
        self.forget_allodia_record(&id);
        self.registry.remove(&id);
        self.refresh_analytics_accounts();
        // Drop it from the reconnect queue too, so a pending retry doesn't try to re-dial an
        // account the user just removed (the in-flight-plan case is caught by the registry
        // re-check in `reconnect_all`).
        self.disconnected
            .lock()
            .expect("disconnected mutex poisoned")
            .remove(&id);
        // Abort the account's push watches / poll timer (apply with no row stops it).
        self.background.apply(&id, None);
        if let Ok(account_id) = AccountId::try_from(id.as_str()) {
            let app = Arc::clone(&self.app);
            self.runtime
                .block_on(async move { app.remove_account(&account_id).await });
            // Removal dropped the account from the MCP exposure list; re-apply so a *running*
            // server stops serving it immediately rather than at the next restart: the config it
            // holds is a snapshot, so nothing else would tell it.
            self.refresh_mcp();
        }
        // Last, and by the core rather than the host: the stored credential. Erasing it before
        // the runtime removal would leave a live account with no way back if the process died in
        // between; doing it here means the only bad outcome left is a stored credential with no
        // account, which the next launch shows as an account that will not connect; visible,
        // and reported below rather than silent.
        self.delete_stored_credential(&id).inspect_err(|err| {
            log::error!(
                "credentials: [{}] the account was removed from the app, but its stored \
                 credential could NOT be erased ({err}); it will come back as an account at the \
                 next launch",
                mailcal_account::account_log_handle(&id),
            );
        })
    }
}
