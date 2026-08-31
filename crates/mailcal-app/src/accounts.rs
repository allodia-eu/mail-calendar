//! Multi-account lifecycle: adding, removing, and taking cheap handles to the accounts the
//! app drives over the shared engine.
//!
//! Split out of `lib.rs` to keep it under the size limit; an `impl App` block reusing the
//! runtime's fields. The `Arc`'d account handles let a caller clone one out under the read
//! guard and drop the guard before any network `.await`, so a long provider round-trip never
//! stalls a concurrent `add_account`.

use std::{sync::Arc, time::Instant};

use engine_api::{AccountId, ContactsProvider, EmailAddress, Provider};
use mailcal_viewmodel::AccountRow;

use crate::{App, Surface, form_factor::FormFactor, scope::Scope};

/// How much history a newly added account syncs.
///
/// Deeper on a desktop than on a phone: a first sync is the longest wait the app asks for and
/// the largest thing it writes, and neither cost lands the same way on mains power and a disk
/// with room as it does on a battery and storage bought by the gigabyte
/// ([`FormFactor`](crate::form_factor::FormFactor)). The user moves it either way afterwards;
/// this is only where the slider starts.
fn new_account_sync_depth_months() -> u16 {
    FormFactor::current().default_sync_depth_months()
}

/// One configured account the app drives, all over the shared engine.
pub struct Account<P> {
    /// The account's id (scopes everything in the shared engine store).
    pub id: AccountId,
    /// The mail providers, one per synced folder (INBOX + role folders).
    pub providers: Vec<P>,
    /// The calendar providers (one per synced calendar), if calendar is configured.
    pub calendar_providers: Vec<P>,
    /// The contact providers: one per address book for CardDAV (source-bound adapters),
    /// exactly one for JMAP (account-global), empty when the account has no contacts.
    ///
    /// Concretely boxed rather than `Vec<P>` because a contacts adapter is a **different**
    /// trait object: `ContactsProvider` is a subtrait of `Provider`, and the two boxes are
    /// distinct types. Making the whole `App` generic over a second parameter to express
    /// that would ripple through every call site for no gain, nothing here needs the
    /// contacts adapter to be the same concrete type as the mail one.
    pub contact_providers: Vec<Box<dyn ContactsProvider>>,
    /// The from-address this account sends as.
    pub identity: EmailAddress,
}

impl<P> core::fmt::Debug for Account<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The providers may hold sensitive connection handles; show only the identity.
        f.debug_struct("Account")
            .field("id", &self.id)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl<P: Provider> App<P> {
    /// Adds an already-connected `account` at runtime, syncs just it, then rebuilds the
    /// snapshot so its mail appears. The host connects the providers off the runtime (the
    /// IMAP login blocks) and hands them here.
    pub async fn add_account(&self, account: Account<P>) {
        let id = account.id.clone();
        let replaced = self.install_account(account).await;
        if !replaced {
            self.set_account_sync_depth(id.as_str(), new_account_sync_depth_months())
                .await;
        }
        if replaced {
            let _ = self.engine.clear_mail_cursors(&id).await;
        }
        let start = Instant::now();
        self.sync_account(&id).await;
        self.rebuild_snapshot().await;
        log::info!(
            "add_account: synced + rebuilt in {}ms",
            start.elapsed().as_millis(),
        );
        self.observer.surface_changed(Surface::Settings);
    }

    /// Registers an account and refreshes the snapshot so it appears in the switcher
    /// **immediately**, WITHOUT syncing it: the caller drives the (possibly slow) first
    /// sync separately, e.g. in the background via [`refresh_account`](Self::refresh_account).
    /// Used for a provider whose initial sync is large (a Microsoft mailbox), so account
    /// creation isn't blocked on downloading everything.
    pub async fn add_account_deferred(&self, account: Account<P>) {
        self.install_account(account).await;
        self.rebuild_snapshot().await;
        self.observer.surface_changed(Surface::Settings);
    }

    /// Puts `account` into the switcher; replacing an existing entry **in place**, else appending
    /// , and reports whether it replaced one.
    ///
    /// The position is the contract. Account ids are unique here, so re-adding one (a reconnect, a
    /// credential change) replaces its providers rather than duplicating it; what it must not do is
    /// move it. Every client renders `account_rows` verbatim, and an interactive launch replaces
    /// *every* account as its background dial lands; three at a time, so in network-latency order.
    /// Removing and re-appending therefore re-sorted the whole switcher once per account on every
    /// boot, visibly, for as long as the slowest dial took.
    ///
    /// A genuinely new account appends, which is the order the host's secure store already keeps:
    /// an ordered account index that appends on first add and leaves every existing entry where
    /// it is.
    async fn install_account(&self, account: Account<P>) -> bool {
        let mut accounts = self.accounts.write().await;
        let at = accounts
            .iter()
            .position(|existing| existing.id == account.id);
        if let Some(at) = at {
            accounts[at] = Arc::new(account);
            true
        } else {
            accounts.push(Arc::new(account));
            false
        }
    }

    /// Registers a newly-added user account without syncing, then gives it the product default
    /// three-month depth before its first background sync starts.
    pub async fn add_new_account_deferred(&self, account: Account<P>) {
        let id = account.id.clone();
        self.add_account_deferred(account).await;
        self.set_account_sync_depth(id.as_str(), new_account_sync_depth_months())
            .await;
    }

    /// Removes the account `id` at runtime: drops it from the switcher, clears it as the
    /// selected account (falling back to the unified "all inboxes") and forgets its cached
    /// messages and optimistic hints, forgets its engine data, then rebuilds the snapshot so its
    /// mail disappears. The host stops the account's background sync and deletes its stored
    /// credential from the OS secure store separately (the bindings layer orchestrates both). A
    /// no-op if no such account is configured.
    pub async fn remove_account(&self, id: &AccountId) {
        {
            let mut accounts = self.accounts.write().await;
            accounts.retain(|existing| existing.id != *id);
        }
        {
            let mut scope = self.scope.lock().expect("scope mutex poisoned");
            if scope.names(id) {
                *scope = Scope::AllInboxes;
            }
        }
        // Drop the account's per-account bookkeeping so a later re-add starts clean.
        let acct = id.as_str();
        self.remove_sync_settings(acct);
        // Including its calendar decisions (colour overrides, hidden calendars); otherwise a
        // re-add inherits a colour the user thought removal had cleared, and never gets the
        // fresh distinct-hue default.
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .remove_account(acct);
        // …and its signature assignment, for the same reason: a re-add must not inherit a pointer
        // to a signature the user may have deleted meanwhile (docs/signatures.md).
        self.remove_account_signature(acct);
        // …and its alias list: that set decides which iTIP `ATTENDEE` line is "me"
        // (docs/invitations.md), so an inherited one would not merely linger; it could make
        // somebody else's invitation on a re-added id read as an RSVP owed by this account.
        self.remove_account_aliases(acct);
        // …and its standing "yes, email the organiser for me" choice, which is permission to send
        // mail as the user and must not be inherited by a re-added id (docs/invitations.md).
        self.remove_reply_fallback(acct);
        // …and whether its folder tree was shut, so a re-add opens showing its folders rather
        // than inheriting a collapse nobody remembers making (docs/folder-pane.md).
        self.remove_account_expansion(acct);
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .retain(|(a, _)| a != acct);
        self.attempted_folders
            .lock()
            .expect("attempted-folders mutex poisoned")
            .retain(|(a, _)| a != acct);
        self.inbox_keys
            .lock()
            .expect("inbox-key mutex poisoned")
            .remove(acct);
        // Clear any outage badge/detail for the removed account, so its connection-issues banner
        // entry doesn't linger forever: only a reachable re-sync would otherwise remove it, which
        // a removed account never gets (leaving a "can't connect: removed@…" banner until restart).
        self.set_account_reachable(id, true);
        // Likewise clear any calendar-reauth prompt, so removing a scope-missing account also
        // dismisses its "reconnect for calendar" banner.
        self.clear_calendar_reauth_required(id);
        // …and any mail write/send re-consent prompt, for the same reason.
        self.clear_mail_reauth_required(id);
        // …and any expired-sign-in prompt: removing the account *is* one way to resolve it, and
        // only a successful sync would otherwise retract it, which a removed account never gets.
        self.clear_signin_expired(id);
        // …and drop it from the MCP exposure list, so an account later re-added under the same
        // id does not silently inherit an exposure the user granted to a different mailbox.
        self.forget_mcp_account(acct);
        self.invalidate_list_cache();
        if self.engine.forget_account(id).await.is_err() {
            log::warn!("remove_account: engine forget failed");
        }
        // Forgetting the account drops its rows but not the raw sources it cached: those are
        // named by a content hash a *different* account's copy may still share, so no row
        // delete can free them. Removing an account is the largest bulk removal there is, so
        // sweep the ones this account was the last to name.
        self.reclaim_freed_space("remove-account").await;
        self.rebuild_snapshot().await;
        self.observer.surface_changed(Surface::Settings);
    }

    /// Renders the mailbox-list snapshot from the **already-persisted** store, without
    /// syncing: so a host can show cached mail the instant it boots, before the background
    /// sync runs. The store survives across launches, so on a returning user this paints the
    /// last-synced inbox immediately; the host then dispatches
    /// [`Intent::RefreshMail`](crate::Intent::RefreshMail), which syncs in the background and
    /// re-renders. Avoids the blank list while a full re-snapshot downloads. A no-op-shaped
    /// first run (empty store) simply renders an empty list.
    pub async fn prime_snapshot(&self) {
        self.rebuild_snapshot().await;
    }

    /// The ids of every configured account.
    pub(crate) async fn account_ids(&self) -> Vec<AccountId> {
        self.accounts
            .read()
            .await
            .iter()
            .map(|account| account.id.clone())
            .collect()
    }

    /// A cheap clone of one account's handle, taken under the read guard and returned so
    /// the caller can use it **after the guard is dropped**; keeping no lock held across a
    /// network `.await`. `None` if no account has that id.
    pub(crate) async fn account_handle(&self, id: &AccountId) -> Option<Arc<Account<P>>> {
        self.accounts
            .read()
            .await
            .iter()
            .find(|account| &account.id == id)
            .map(Arc::clone)
    }

    /// The account's position in the stored list, for the diagnostic log.
    ///
    /// **Stable across passes, which a per-pass ordinal is not.** A pass over every account
    /// numbered them as it went, so a single-account pass: a watch, an on-demand open, an
    /// account just added; always called itself `a0`, and a log with several accounts in it
    /// could not say which one a line belonged to. Position is what the sidebar shows in the
    /// same order, so `a2` in the log is the third account on screen.
    ///
    /// An address can never appear in a log line (`docs/logging.md`), which is what rules out
    /// the obvious alternative. `usize::MAX` for an account that is no longer stored: a sync
    /// finishing just after a removal, which is rare and better labelled oddly than dropped.
    pub(crate) async fn account_ordinal(&self, id: &AccountId) -> usize {
        self.accounts
            .read()
            .await
            .iter()
            .position(|account| &account.id == id)
            .unwrap_or(usize::MAX)
    }

    /// Cheap clones of every account handle, for the same reason as [`Self::account_handle`]
    /// ; iterate and do network I/O over these with the read guard already released.
    pub(crate) async fn account_handles(&self) -> Vec<Arc<Account<P>>> {
        self.accounts.read().await.iter().map(Arc::clone).collect()
    }

    /// The sidebar switcher rows (id + email) for every configured account, in the order the host
    /// stored them; see `install_account` for what holds that order across a reconnect.
    pub(crate) async fn account_rows(&self) -> Vec<AccountRow> {
        self.accounts
            .read()
            .await
            .iter()
            .map(|account| AccountRow {
                id: account.id.as_str().to_owned(),
                email: account.identity.email.clone(),
                expanded: self.account_expanded(account.id.as_str()),
            })
            .collect()
    }
}
