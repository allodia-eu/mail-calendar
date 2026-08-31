//! Calendar agenda operations across every account's calendars: sync over the horizon,
//! rebuild the merged agenda in the active display zone, and create/delete events through
//! the outbox. Split out of `lib.rs` to keep it under the 500-line limit; an `impl App`
//! block here reuses the runtime's fields and the active-zone accessor.

use std::collections::{HashMap, HashSet};

use engine_api::{AccountId, Event, LocalDateTime, Provider, ProviderKey};
use mailcal_viewmodel::calendar::{self, AccountEvent};

use crate::{
    Account, App, CalendarWriteStatus,
    calendar_cache::rolling_horizon,
    helpers::{generated_idempotency, generated_uid, now_utc},
    reference::EventRef,
};

impl<P: Provider> App<P> {
    /// Paints the grid from what the store **already has**: no network, no expansion.
    ///
    /// The mail list has done this since the beginning ([`App::prime_snapshot`]): the store
    /// survives across launches, so a returning user sees their last-synced mail the instant the
    /// app boots, and the background sync merely corrects it.
    ///
    /// The calendar did not, and the omission was invisible in every test and brutal in the hand:
    /// the cache starts empty, and *nothing built it but `App::refresh_calendar`*, which syncs
    /// every calendar over the network first. So opening the calendar meant staring at "loading
    /// this period…" for as long as CalDAV took to answer, over a store that had held the answer
    /// all along. The grid is a pull from this cache; if the cache is cold, the grid is blank.
    ///
    /// Cheap by construction: local reads only. It is on the boot path, so it must stay that way.
    ///
    /// On a store that has **never** been synced it deliberately does nothing. Priming it would set
    /// the cache window, and `is_materialized` would flip to `true` over an empty store: so a
    /// first-run user would be shown a confidently empty week instead of "loading this period…".
    /// That is the one lie `docs/calendar.md` exists to forbid: `false` means *we have not looked*,
    /// and it must not be rendered as *there is nothing there*.
    pub async fn prime_calendar(&self) {
        let Some(horizon) = rolling_horizon() else {
            return;
        };
        if !self.has_stored_calendars().await {
            return;
        }
        if self.rebuild_calendar_cache(horizon).await {
            self.rebuild_calendar().await;
        }
    }

    /// Whether the store has ever seen this user's calendars.
    ///
    /// A synced calendar account always has at least one collection, so "no calendars at all" is
    /// the honest signal for *we have never synced*, as distinct from *you have nothing on*.
    async fn has_stored_calendars(&self) -> bool {
        for id in self.account_ids().await {
            if !self
                .engine
                .calendars(&id)
                .await
                .unwrap_or_default()
                .is_empty()
            {
                return true;
            }
        }
        false
    }

    /// Projects the events in the materialized window into the agenda, ordered by absolute
    /// instant and localised in the active display zone; then signals [`Surface::Calendar`].
    /// Used after a calendar sync and after a display-zone change (the latter needs no network
    /// round-trip, just a re-projection).
    ///
    /// The agenda is the **same windowed set the grid draws**, not every event the store has
    /// ever held. A real diary is ~10,000 events, of which a few hundred are live in the
    /// rolling window; listing all of them merely to sort them soonest-first cost a full
    /// `events()` decode per account here *and* a ~10,000-row reconcile on the host's UI thread
    /// every refresh. The window's event keys are read straight off the in-memory grid cache
    /// ([`Self::windowed_event_keys`]) and resolved in one targeted read each, so this touches
    /// only what the user could actually be shown.
    pub(super) async fn rebuild_calendar(&self) {
        let zone = self.active_zone();
        let wanted = self.windowed_event_keys();
        let mut events = Vec::new();
        for account in self.account_handles().await {
            let can_write = Self::account_can_write(&account);
            let account_id = account.id.as_str().to_owned();
            let keys = wanted.get(&account_id).map_or(&[][..], Vec::as_slice);
            // The account's own addresses, so an unanswered hold reads as one in the agenda too.
            // Declined events cannot reach here at all: `keys` comes from the occurrence cache,
            // which already dropped them; one hiding rule, one place.
            let addresses = self.account_address_set(&account.id).await;
            events.extend(
                self.engine
                    .events_by_keys(&account.id, keys)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|event| AccountEvent {
                        account: account_id.clone(),
                        participation: crate::invitations::diary_participation(&event, &addresses),
                        event,
                        can_write,
                    }),
            );
        }
        self.calendar.publish(calendar::build(&events, &zone));
    }

    /// The distinct event keys with an occurrence in the materialized window, grouped by
    /// account; read off the in-memory grid cache so the agenda projects exactly the set the
    /// grid draws, with no second store scan. Empty before the first cache build (a store that
    /// has never synced), which yields an empty agenda rather than the whole store.
    fn windowed_event_keys(&self) -> HashMap<String, Vec<ProviderKey>> {
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        let mut by_account: HashMap<String, HashSet<ProviderKey>> = HashMap::new();
        for occurrence in &cache.occurrences {
            if let Ok(key) = ProviderKey::new(occurrence.event.clone()) {
                by_account
                    .entry(occurrence.account.clone())
                    .or_default()
                    .insert(key);
            }
        }
        by_account
            .into_iter()
            .map(|(account, keys)| (account, keys.into_iter().collect()))
            .collect()
    }

    /// Rebuilds the grid cache and the agenda from the store, **without** syncing.
    ///
    /// Call this after a write: the store is already fresh when the engine's write
    /// call returns, so a second sync would only waste a network round-trip.
    pub(crate) async fn rebuild_calendar_view(&self) {
        let Some(horizon) = rolling_horizon() else {
            return;
        };
        if self.rebuild_calendar_cache(horizon).await {
            self.rebuild_calendar().await;
        }
    }

    /// Creates a calendar event via the engine's outbox (awaited inline; there is no
    /// background drainer yet), then rebuilds the agenda from the store. The `mailcal-account`
    /// glue builds the intent (`EventDraft`); the adapter serializes it, so the app needs no
    /// `provider-caldav` dependency.
    ///
    /// `account`/`calendar` are the client's picker choice: the owning account id and the
    /// calendar's row key (`CalendarRow.id`). The event lands in that calendar when the account
    /// can write; otherwise it falls back to the default writable account
    /// ([`Self::create_account`]) and its first calendar, and to that account's chosen calendar
    /// if the passed key is unknown there. `all_day` selects the event form; `timezone` (when set)
    /// creates a timed event in that zone rather than UTC; `notes` its description; `location` its
    /// place; `recurrence` makes it repeat. A no-op if no account can write, the account has no
    /// calendar, or the fields are invalid (the skeleton swallows it).
    // A flat pass-through of `Intent::CreateEvent`'s fields; each a distinct scalar from the
    // create form. A parameter struct would only re-wrap the same values `dispatch` just
    // destructured, so it stays flat.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn create_event(
        &self,
        title: String,
        start: String,
        end: String,
        account: Option<String>,
        calendar: Option<String>,
        all_day: bool,
        timezone: Option<String>,
        notes: Option<String>,
        location: Option<String>,
        recurrence: Option<mailcal_account::SimpleRecurrence>,
    ) {
        let Some(account) = self.create_account(account).await else {
            return;
        };
        let calendars = self.engine.calendars(&account).await.unwrap_or_default();
        // The chosen calendar, matched by the row key the picker sends (`CalendarRow.id` is a
        // calendar's `key()`), else the account's first calendar.
        let Some(calendar) = calendar
            .as_deref()
            .and_then(|key| calendars.iter().find(|c| c.id.key().as_str() == key))
            .or_else(|| calendars.first())
        else {
            return;
        };
        let Ok(stamp) = now_utc() else {
            return;
        };
        let uid = generated_uid();
        let Ok(draft) = mailcal_account::build_event_draft(
            calendar.id.clone(),
            &uid,
            &title,
            &start,
            &end,
            all_day,
            timezone.as_deref(),
            notes.as_deref(),
            location.as_deref(),
            recurrence.as_ref(),
            stamp,
        ) else {
            return;
        };
        // Clone the account handle, then write with the read guard released.
        let mut status = None;
        if let Some(acct) = self.account_handle(&account).await
            && let Some(provider) = acct.calendar_providers.first()
        {
            self.set_calendar_write_status(CalendarWriteStatus::Saving);
            let write = self
                .engine
                .create_calendar_event(provider, &account, &generated_idempotency(), &draft)
                .await;
            status = Some(match write {
                Ok(write) => {
                    self.settle_calendar_write(provider, &account, write.reconciled)
                        .await
                }
                Err(err) => {
                    log::warn!(
                        "create_calendar_event failed for [{}]: {err}",
                        mailcal_account::account_log_handle(account.as_str()),
                    );
                    CalendarWriteStatus::Failed
                }
            });
        }
        self.rebuild_calendar_view().await;
        if let Some(status) = status {
            self.set_calendar_write_status(status);
        }
    }

    /// Edits a stored calendar event (retitle, move, or resize) by **patching the stored
    /// provider payload**, then rebuilding the agenda from the store. The
    /// `mailcal-account` glue turns the edit into a provider-neutral [`EventPatch`]; the
    /// adapter applies it, so only what the user changed changes and the recurrence rule,
    /// attendees, alarms and timezone survive.
    ///
    /// Unlike its neighbours here, this **returns its failure** rather than swallowing it.
    /// The write is still awaited inline with no outbox drainer behind it, so a failed edit
    /// is simply a failed edit; the caller must not report it as saved.
    ///
    /// # Errors
    ///
    /// Returns the reason the edit did not happen: the event is not in the store, its
    /// account has no calendar provider, the edit names an occurrence the series does not
    /// have, the event cannot be patched, the edit is invalid, or the provider call itself
    /// failed.
    pub(super) async fn update_event(
        &self,
        event: &EventRef,
        edit: &mailcal_account::EventEdit,
    ) -> Result<(), String> {
        let Some(stored) = self.stored_event(event).await else {
            return Err(format!("no event {:?} in the store", event.key.as_str()));
        };
        if let Some(occurrence) = edit.occurrence
            && !self
                .names_a_stored_occurrence(event, &stored, occurrence)
                .await
        {
            self.set_calendar_write_status(CalendarWriteStatus::Failed);
            return Err("the edit names no occurrence of this series".to_owned());
        }

        let (target, patch) = mailcal_account::build_event_patch(&stored, edit, now_utc()?)
            .map_err(|err| {
                // Refused before anything was sent: a repeat rule this app could not describe
                // in full, or a wall clock the stored event's form cannot hold. Nothing
                // happened, and the user must not be left reading an unchanged event with no
                // sign that their save did not take.
                self.set_calendar_write_status(CalendarWriteStatus::Failed);
                err.to_string()
            })?;

        // Clone the account handle, then write with the read guard released.
        let acct = self
            .account_handle(&event.account)
            .await
            .ok_or_else(|| format!("account {:?} is not configured", event.account.as_str()))?;
        let provider = acct
            .calendar_providers
            .first()
            .ok_or_else(|| "the account has no calendar provider".to_owned())?;
        self.set_calendar_write_status(CalendarWriteStatus::Saving);
        let write = self
            .engine
            .patch_calendar_event(
                provider,
                &event.account,
                &generated_idempotency(),
                &stored,
                target,
                patch,
            )
            .await
            .map_err(|err| {
                // The server call itself failed: the edit may not have landed. Surface it.
                self.set_calendar_write_status(CalendarWriteStatus::Failed);
                err.to_string()
            })?;

        let status = self
            .settle_calendar_write(provider, &event.account, write.reconciled)
            .await;
        self.rebuild_calendar_view().await;
        self.set_calendar_write_status(status);
        Ok(())
    }

    /// Deletes the calendar `event` from its owning account via the engine's outbox (awaited
    /// inline; there is no background drainer yet), then rebuilds the agenda from the store.
    /// Routing by the reference's account is the fix:
    /// event keys are only unique within an account, so two accounts can mint the same key.
    ///
    /// `occurrence` names the single instance to remove, by its **original** start; `None`
    /// deletes the whole series.
    pub(super) async fn delete_event(&self, event: EventRef, occurrence: Option<LocalDateTime>) {
        let Some(stored) = self.stored_event(&event).await else {
            return;
        };
        if let Some(named) = occurrence
            && !self.names_a_stored_occurrence(&event, &stored, named).await
        {
            // Refused rather than widened. A delete that could not find the occurrence and
            // removed the series instead is the one outcome nobody can undo.
            log::warn!(
                "delete_calendar_event: no such occurrence for {}",
                event.account.as_str()
            );
            self.set_calendar_write_status(CalendarWriteStatus::Failed);
            return;
        }
        let built = now_utc().and_then(|stamp| {
            mailcal_account::build_event_deletion(&stored, occurrence, stamp)
                .map_err(|err| err.to_string())
        });
        let deletion = match built {
            Ok(deletion) => deletion,
            Err(err) => {
                // Unlike the whole-series delete this cannot simply fall through: the user
                // asked for one occurrence to go, and returning quietly would leave it on
                // the grid with nothing anywhere saying the delete never happened.
                log::warn!(
                    "delete_calendar_event: the delete was not built for {}: {err}",
                    event.account.as_str()
                );
                self.set_calendar_write_status(CalendarWriteStatus::Failed);
                return;
            }
        };
        // Clone the account handle, then delete with the read guard released.
        let mut status = None;
        if let Some(acct) = self.account_handle(&event.account).await
            && let Some(provider) = acct.calendar_providers.first()
        {
            self.set_calendar_write_status(CalendarWriteStatus::Saving);
            let delete = self
                .engine
                .delete_calendar_event(
                    provider,
                    &event.account,
                    &generated_idempotency(),
                    // The stored event, because removing **one occurrence** of a series is an
                    // edit of the stored document on CalDAV: the adapter rewrites those bytes
                    // and needs them. Passed for a whole-event delete too; a transport that
                    // deletes an object outright ignores it.
                    Some(&stored),
                    &deletion,
                )
                .await;
            status = Some(match delete {
                Ok(delete) => {
                    self.settle_calendar_write(provider, &event.account, delete.reconciled)
                        .await
                }
                Err(err) => {
                    log::warn!(
                        "delete_calendar_event failed for {}: {err}",
                        event.account.as_str()
                    );
                    CalendarWriteStatus::Failed
                }
            });
        }
        self.rebuild_calendar_view().await;
        if let Some(status) = status {
            self.set_calendar_write_status(status);
        }
    }

    /// Looks up the stored [`Event`] an [`EventRef`] names in its owning account. Shared with
    /// the event-detail read ([`crate::calendar_detail`]).
    ///
    /// A **targeted** read: it resolves the one named key rather than decoding the account's
    /// whole event history to scan for it. That distinction is the difference between a
    /// tap-to-open that is instant and one that stalls for seconds; on a real diary
    /// [`engine_api::Engine::events`] deserializes every one of thousands of event payloads,
    /// and this runs on the boot-blocking-free but user-facing detail path, once per tap.
    pub(crate) async fn stored_event(&self, event: &EventRef) -> Option<Event> {
        self.engine
            .events_by_keys(&event.account, std::slice::from_ref(&event.key))
            .await
            .unwrap_or_default()
            .into_iter()
            .next()
    }

    /// Whether `account`'s calendar provider advertises writes: the same predicate that
    /// stamps `can_write` onto every row the account contributes. Shared with the
    /// event-detail read ([`crate::calendar_detail`]).
    pub(crate) fn account_can_write(account: &Account<P>) -> bool {
        account.calendar_providers.first().is_some_and(|provider| {
            provider
                .connection_info()
                .capabilities
                .calendar_write_guard()
                .is_some()
        })
    }

    /// The account a new event is created in: the selected account when its calendar can be
    /// written, else the first account whose calendar can: the same `can_write` rule a
    /// client disables its create affordance on, so an enabled "New event" and the routed
    /// write cannot disagree. `None` (the create is a no-op) when no account can write.
    async fn calendar_account(&self) -> Option<AccountId> {
        let selected = self
            .scope
            .lock()
            .expect("scope mutex poisoned")
            .account()
            .cloned();
        let accounts = self.accounts.read().await;
        if let Some(selected) = selected
            && accounts
                .iter()
                .any(|a| a.id == selected && Self::account_can_write(a))
        {
            return Some(selected);
        }
        accounts
            .iter()
            .find(|a| Self::account_can_write(a))
            .map(|a| a.id.clone())
    }

    /// The account a create is routed to: the client's explicit picker choice when it is a
    /// configured, **writable** account, else the default ([`Self::calendar_account`]). Keeping
    /// the fallback on the same `can_write` rule means an enabled "New event" and the routed
    /// write cannot disagree: a picker choice that has since gone read-only still lands
    /// somewhere writable rather than failing. `None` when no account can write.
    async fn create_account(&self, chosen: Option<String>) -> Option<AccountId> {
        if let Some(chosen) = chosen {
            let writable = self
                .accounts
                .read()
                .await
                .iter()
                .find(|a| a.id.as_str() == chosen && Self::account_can_write(a))
                .map(|a| a.id.clone());
            if writable.is_some() {
                return writable;
            }
        }
        self.calendar_account().await
    }
}
