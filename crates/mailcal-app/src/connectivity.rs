//! Connectivity: the device-online signal plus per-account reachability, surfaced via
//! [`Surface::Connectivity`] and rendered by a host as an offline banner and per-account
//! outage badges.
//!
//! A second `impl App` block (like `reading` / `calendar_ops`) so `lib.rs` stays small. The
//! app assumes it is **online** until the host reports otherwise
//! ([`Intent::ReportNetworkReachable`](crate::Intent::ReportNetworkReachable)), so a host
//! that never wires reachability behaves exactly as before.

use std::error::Error as StdError;

use engine_api::{AccountId, ApiError, Provider};
use engine_core::error::FailureClass;
use engine_provider::{ConnectionInfo, ProviderError};
use mailcal_viewmodel::ConnectivitySnapshot;
use tokio::sync::watch;

use crate::{App, protocol::Surface, sync::RefreshProgress};

impl<P: Provider> App<P> {
    /// Whether the device currently has network connectivity. The app's network syncs check
    /// this and short-circuit when offline, so an overnight lidless wake on a dead network
    /// doesn't storm it with doomed reconnects.
    pub(crate) fn is_online(&self) -> bool {
        *self.online.borrow()
    }

    /// A receiver the background watch/poll loops subscribe to, so they can **await** the
    /// return to online instead of busy-retrying while the device is offline. Handed to the
    /// bindings' background manager at boot.
    #[must_use]
    pub fn online_signal(&self) -> watch::Receiver<bool> {
        self.online.subscribe()
    }

    /// The current connectivity snapshot (pulled after a [`Surface::Connectivity`] signal):
    /// the device-offline flag and, while online, the accounts whose last sync couldn't
    /// reach their server.
    #[must_use]
    pub fn connectivity(&self) -> ConnectivitySnapshot {
        let offline = !self.is_online();
        let signin_expired_accounts: Vec<String> = self
            .signin_expired_accounts
            .lock()
            .expect("signin-expired mutex poisoned")
            .iter()
            .cloned()
            .collect();
        // While offline the fault is the device's, not any one account's; suppress the
        // per-account badges so they don't double up with the global banner. An account with a
        // dead grant is filtered out too: it has its own, more specific prompt, and "can't reach
        // this account's server" alongside "your sign-in expired" would contradict itself.
        let unreachable_accounts = if offline {
            Vec::new()
        } else {
            self.unreachable_accounts
                .lock()
                .expect("unreachable-accounts mutex poisoned")
                .keys()
                .filter(|id| !signin_expired_accounts.contains(id))
                .cloned()
                .collect()
        };
        ConnectivitySnapshot {
            offline,
            unreachable_accounts,
            // A standing permission gap, not a connectivity fault; surfaced regardless of the
            // offline state (re-consent is the remedy, and the state is real either way).
            calendar_reauth_accounts: self
                .calendar_reauth_accounts
                .lock()
                .expect("calendar-reauth mutex poisoned")
                .iter()
                .cloned()
                .collect(),
            // Like the calendar prompt, a standing permission gap (the grant lacks the mail
            // write/send scopes); surfaced regardless of offline state, since re-consent is the
            // remedy and the state is real either way.
            mail_reauth_accounts: self
                .mail_reauth_accounts
                .lock()
                .expect("mail-reauth mutex poisoned")
                .iter()
                .cloned()
                .collect(),
            // A dead grant, not an outage (the server answered and refused the credential) so
            // it survives going offline like the two prompts above: re-consent is the remedy
            // whether or not the device has a network right now.
            signin_expired_accounts,
        }
    }

    /// Marks `account` as needing calendar re-authentication (its OAuth grant lacks the calendar
    /// scope), signalling [`Surface::Connectivity`] only when the set actually changes. Mail is
    /// unaffected: a host shows a "reconnect to enable calendar" prompt, not an outage badge.
    pub fn note_calendar_reauth_required(&self, account: &AccountId) {
        let changed = self
            .calendar_reauth_accounts
            .lock()
            .expect("calendar-reauth mutex poisoned")
            .insert(account.as_str().to_owned());
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Clears `account`'s calendar-reauth flag; after a successful calendar connect (e.g. the
    /// user re-authenticated and the new grant carries the scope), or on account removal.
    /// Signals [`Surface::Connectivity`] only when the set actually changes.
    pub fn clear_calendar_reauth_required(&self, account: &AccountId) {
        let changed = self
            .calendar_reauth_accounts
            .lock()
            .expect("calendar-reauth mutex poisoned")
            .remove(account.as_str());
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Marks `account` as needing a mail **re-authentication**: a mark-read/flag/move/delete or a
    /// send was refused with a Graph `403 ErrorAccessDenied`, so the OAuth grant lacks the mail
    /// write/send scopes (`Mail.ReadWrite` / `Mail.Send`); connected before those scopes, or
    /// consent revoked server-side. Mail **reading** is unaffected; a host shows a "reconnect to
    /// send and manage mail" prompt whose action re-runs the account's OAuth sign-in (which
    /// re-grants the whole scope set). Signals [`Surface::Connectivity`] only when the set
    /// actually changes and, on that first raise, logs the cause **privacy-safely**: the count
    /// and reason, never the account address: so a support log shows why the prompt appeared.
    /// Sibling of [`note_calendar_reauth_required`](Self::note_calendar_reauth_required).
    pub(crate) fn note_mail_reauth_required(&self, account: &AccountId) {
        let raised = {
            let mut set = self
                .mail_reauth_accounts
                .lock()
                .expect("mail-reauth mutex poisoned");
            set.insert(account.as_str().to_owned()).then(|| set.len())
        };
        if let Some(count) = raised {
            log::warn!(
                "mail: a mail write or send was refused for lack of permission (Graph \
                 Mail.ReadWrite/Mail.Send not granted); raising the reconnect-to-send prompt; \
                 {count} account(s) now awaiting mail re-consent",
            );
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Clears `account`'s mail-reauth flag; after a successful mail write/send (the grant plainly
    /// works now), after a re-consent completes, or on account removal. Signals
    /// [`Surface::Connectivity`] only when the set actually changes, logging the recovery so the
    /// support trail shows the prompt lifting. `pub` so the bindings can clear it when a
    /// re-authentication completes (mirroring the calendar path).
    pub fn clear_mail_reauth_required(&self, account: &AccountId) {
        let cleared = self
            .mail_reauth_accounts
            .lock()
            .expect("mail-reauth mutex poisoned")
            .remove(account.as_str());
        if cleared {
            log::info!(
                "mail: mail write/send permission confirmed for an account; clearing its \
                 reconnect-to-send prompt",
            );
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Reconciles `account`'s mail-reauth prompt from a **failed** mail write/send: raises it only
    /// when the failure is a Graph insufficient-permissions `403`
    /// ([`is_graph_permission_denied`]). Any other failure (transient, a conflict, a different
    /// `403` such as `ErrorCannotDeleteObject` on an idempotent re-delete) leaves the flag
    /// untouched: a retry, not a re-consent, is the remedy there. The success side is handled by
    /// the caller clearing the flag directly.
    pub(crate) fn note_mail_write_error(&self, account: &AccountId, error: &ApiError) {
        // `&ApiError` coerces to `&dyn Error`; the classifier walks its `source()` chain.
        if is_graph_permission_denied(error) {
            self.note_mail_reauth_required(account);
        } else {
            // The write/send failed for some *other* reason (transient, a conflict, a non-auth
            // `403`): a retry is the remedy, not a re-consent, so the prompt stays as-is. Logged
            // at debug (the caller already logs the failure itself at warn) so a support session
            // can tell "we classified this as not-a-permission-gap" apart from "the classifier
            // missed a real `ErrorAccessDenied`" when a user reports the banner never appeared.
            log::debug!(
                "mail: a mail write/send failed but not for lack of permission (no Graph \
                 ErrorAccessDenied in the error chain); leaving the reconnect-to-send prompt \
                 unchanged; a retry is the remedy",
            );
        }
    }

    /// Marks `account`'s stored OAuth grant as **dead**: a sync failed with
    /// [`FailureClass::Authentication`], i.e. the
    /// refresh token expired or was revoked and no longer mints an access token (Google
    /// `invalid_grant`, a Microsoft `AADSTS700082`). Nothing about the account works until the user
    /// signs in again, so a host shows a "your sign-in expired; reconnect" prompt instead of the
    /// outage badge. Signals [`Surface::Connectivity`] only on the first raise and logs the cause
    /// **privacy-safely** (the count and reason, never the address) so a support log shows why
    /// the prompt appeared. Sibling of `note_mail_reauth_required`.
    ///
    /// `pub` because a dead grant is caught at **connect** as often as at sync: the interactive
    /// app dials every account in the background, so a revoked token surfaces in the bindings'
    /// reconnect pass (as a typed `AccountError::SigninRejected`) and never reaches the sync-pass
    /// classifier at all. Mirrors [`clear_signin_expired`](Self::clear_signin_expired) being `pub`.
    pub fn note_signin_expired(&self, account: &AccountId) {
        let raised = {
            let mut set = self
                .signin_expired_accounts
                .lock()
                .expect("signin-expired mutex poisoned");
            set.insert(account.as_str().to_owned()).then(|| set.len())
        };
        if let Some(count) = raised {
            log::warn!(
                "mail: an account's stored sign-in was refused by its server (expired, revoked, or \
                 no longer accepted); raising the reconnect prompt; {count} account(s) now \
                 awaiting a fresh sign-in",
            );
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Clears `account`'s expired-sign-in flag; after any successful sync (the credential plainly
    /// works again), after a re-authentication completes, or on account removal. Signals
    /// [`Surface::Connectivity`] only when the set actually changes, logging the recovery so the
    /// support trail shows the prompt lifting. `pub` so the bindings can clear it when a
    /// re-authentication completes, mirroring the calendar and mail paths.
    pub fn clear_signin_expired(&self, account: &AccountId) {
        let cleared = self
            .signin_expired_accounts
            .lock()
            .expect("signin-expired mutex poisoned")
            .remove(account.as_str());
        if cleared {
            log::info!("mail: an account's sign-in works again; clearing its reconnect prompt");
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Applies one sync pass's expired-sign-in verdict: raise the prompt, clear it, or: for the
    /// `None` the pass reports when it proved nothing either way (every scope busy, or only
    /// transport failures); leave it exactly as it was.
    pub(crate) fn apply_signin_expired(&self, account: &AccountId, expired: Option<bool>) {
        match expired {
            Some(true) => self.note_signin_expired(account),
            Some(false) => self.clear_signin_expired(account),
            None => {}
        }
    }

    /// Records the host's OS reachability report. Going **offline** signals the banner and
    /// stops the app attempting syncs; coming back **online** triggers a refresh, which
    /// re-dials the (now-dead) providers and pulls whatever arrived meanwhile. A report that
    /// doesn't change the state is a no-op; hosts may re-report the same value.
    pub(crate) async fn report_network_reachable(&self, reachable: bool) {
        if self.is_online() == reachable {
            return;
        }
        // `send_replace` updates the value and wakes every subscriber (the watch/poll loops),
        // and (unlike `send`) succeeds even with no receivers yet (the demo / tests, or
        // before the background manager subscribes), so the state always flips.
        self.online.send_replace(reachable);
        self.observer.surface_changed(Surface::Connectivity);
        if reachable {
            // Back online: the reconnecting providers re-dial on their next call, so this
            // refresh both reconnects them and catches up on mail missed while offline.
            self.refresh_mail(RefreshProgress::Background).await;
        }
    }

    /// Records whether `account` reached its server on its last sync, updating the
    /// per-account outage set and signalling [`Surface::Connectivity`] only when membership
    /// actually changes: so a healthy re-sync of an already-healthy account never churns the
    /// surface. Becoming unreachable here carries no detail (a mid-session sync failure): it
    /// keeps any richer detail a prior boot-connect failure recorded ([`note_account_unreachable`],
    /// via `or_insert`) rather than clobbering it.
    ///
    /// [`note_account_unreachable`]: Self::note_account_unreachable
    pub(crate) fn set_account_reachable(&self, account: &AccountId, reachable: bool) {
        let changed = {
            let mut unreachable = self
                .unreachable_accounts
                .lock()
                .expect("unreachable-accounts mutex poisoned");
            if reachable {
                unreachable.remove(account.as_str()).is_some()
            } else {
                // Newly unreachable if it wasn't already present; preserve any existing detail.
                let was_present = unreachable.contains_key(account.as_str());
                unreachable
                    .entry(account.as_str().to_owned())
                    .or_insert(None);
                !was_present
            }
        };
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// Marks `account` unreachable **with** a technical `detail` (the connect error) a host
    /// reveals behind a "details" link; used to seed the outage set at boot for an account whose
    /// providers couldn't connect. Signals [`Surface::Connectivity`] when the account newly
    /// becomes unreachable or its detail changes, so re-noting the same detail is a no-op.
    pub fn note_account_unreachable(&self, account: &AccountId, detail: Option<String>) {
        let changed = {
            let mut unreachable = self
                .unreachable_accounts
                .lock()
                .expect("unreachable-accounts mutex poisoned");
            match unreachable.get(account.as_str()) {
                Some(existing) if *existing == detail => false,
                _ => {
                    unreachable.insert(account.as_str().to_owned(), detail);
                    true
                }
            }
        };
        if changed {
            self.observer.surface_changed(Surface::Connectivity);
        }
    }

    /// The stored technical detail for `account`'s current outage (the connect error), or `None`
    /// when it is reachable or the outage carries no detail. A host pulls this on a
    /// [`Surface::Connectivity`] signal to fill the per-account "details" view.
    #[must_use]
    pub fn connection_detail(&self, account: &AccountId) -> Option<String> {
        self.unreachable_accounts
            .lock()
            .expect("unreachable-accounts mutex poisoned")
            .get(account.as_str())
            .cloned()
            .flatten()
    }

    /// The connection facts currently reported by `account`'s live providers. Empty means the
    /// account is unknown or has no live providers this session.
    pub async fn connection_info(&self, account: &AccountId) -> Vec<ConnectionInfo> {
        let Some(account) = self.account_handle(account).await else {
            return Vec::new();
        };
        account
            .providers
            .iter()
            .chain(account.calendar_providers.iter())
            .map(Provider::connection_info)
            .collect()
    }
}

/// Whether `error` is Microsoft Graph reporting **insufficient permissions** for a mail write or
/// send; HTTP `403 ErrorAccessDenied`, i.e. the account's OAuth grant lacks
/// `Mail.ReadWrite`/`Mail.Send` (connected before those scopes) or consent was revoked
/// server-side. Re-authenticating (re-consent) is the only remedy; a plain retry never helps.
///
/// Classified **structurally**, not by matching the whole rendered error: the engine's
/// [`ApiError`](engine_api::ApiError) preserves its `source()` chain down to the provider
/// failure, so this walks to the typed [`ProviderError`](engine_provider::ProviderError) and
/// matches the Graph `ErrorAccessDenied` code on *its* detail. That deliberately excludes a
/// different `403` (e.g. `ErrorCannotDeleteObject` on an idempotent re-delete) and any transient
/// failure whose body merely echoes the phrase: only a genuine access-denied refusal flags the
/// account. The calendar side matches the same code (`graph::is_calendar_access_denied`), but on a
/// `ProviderError` it holds directly; here the provider error is nested inside the outbox's
/// `ApiError`, hence the walk. Takes `&dyn Error` (an [`ApiError`](engine_api::ApiError) coerces)
/// so the walk is unit-testable against a hand-built nested chain.
pub(crate) fn is_graph_permission_denied(error: &(dyn StdError + 'static)) -> bool {
    provider_error_of(error).is_some_and(|provider| provider.detail().contains("ErrorAccessDenied"))
}

/// Whether `error` is the account's stored OAuth grant being **rejected outright**: the refresh
/// token expired or was revoked, so it no longer mints an access token (Google `invalid_grant`, a
/// Microsoft `AADSTS700082`, a withdrawn OAuth JMAP token). Re-authenticating is the only remedy;
/// a retry never helps, and the server plainly *was* reached, so this is not an outage.
///
/// Classified **structurally**, on the engine's own
/// [`FailureClass::Authentication`](engine_core::error::FailureClass::Authentication) rather than
/// by matching error text: every provider adapter already maps its own flavour of "your credential
/// is no good" onto that one class, so a new provider is covered the day it is added, and a
/// transient failure whose body merely echoes the phrase is not. That deliberately also catches a
/// **wrong password** on an IMAP account, which is the same fact and the same remedy; a
/// credential the server refuses.
pub(crate) fn is_signin_expired(error: &(dyn StdError + 'static)) -> bool {
    provider_error_of(error)
        .is_some_and(|provider| provider.class() == FailureClass::Authentication)
}

/// Walks `error`'s `source()` chain to the typed [`ProviderError`](engine_provider::ProviderError)
/// underneath, or `None` when the failure never came from a provider (a store or query error). The
/// engine's [`ApiError`](engine_api::ApiError) preserves the chain, so this is what lets the
/// classifiers above match on typed facts instead of on rendered text. Takes `&dyn Error` (an
/// `ApiError` coerces) so the walk is unit-testable against a hand-built nested chain.
fn provider_error_of<'a>(error: &'a (dyn StdError + 'static)) -> Option<&'a ProviderError> {
    let mut source = Some(error);
    while let Some(err) = source {
        if let Some(provider) = err.downcast_ref::<ProviderError>() {
            return Some(provider);
        }
        source = err.source();
    }
    None
}
