//! Assembling an [`InvitationCard`] for the open message: the half that needs the store.
//!
//! The rules live next door in [`crate::invitations`], as pure functions. This module reads: the
//! account's address set, the iTIP payload behind the message, and the user's own diary over the
//! meeting's day so the card can show what it collides with.
//!
//! **Privacy.** A meeting's title, its organiser and its attendees are message content. Nothing
//! here logs any of it; `docs/logging.md` is absolute, and a support log has to stay safe to
//! attach to an email.

use std::collections::{HashMap, HashSet};

use engine_api::{
    AccountId, CalendarDate, Event, Horizon, InboundScheduling, Message, OccurrenceRow, Provider,
    ProviderKey, TimeZoneId, UtcDateTime, day_bounds_utc, resolve_instant_in, to_local,
};
use mailcal_account::load_preferences;
use mailcal_viewmodel::{
    InvitationCard, InvitationKind, ResponseStatus,
    calendar::grid::{self, Occurrence, TimeGrid},
};

use crate::{
    App,
    invitations::{
        Delivery, DiaryEntry, count_conflicts, delivery, description, diary_participation,
        location, matched_attendee, my_response, organizer_line, proposed_hold, summary, tally,
    },
    reference::MessageRef,
};

impl<P: Provider> App<P> {
    /// Builds the invitation card for `message`, or `None` when it is ordinary mail.
    ///
    /// Returns `None` for anything the RSVP gate rejects (`crate::invitations::classify`), so a
    /// published `.ics` produces no card and keeps its attachment chip. Best-effort throughout:
    /// a provider error, an unresolvable time zone, or a diary read that fails yields either no
    /// card or a card with no conflict preview; never a failed message open, because an
    /// invitation the user cannot read is worse than an invitation with no clash summary.
    pub(crate) async fn invitation_card(
        &self,
        message: &MessageRef,
        original: &Message,
    ) -> Option<InvitationCard> {
        let scheduling = self.fetch_scheduling(message, original).await?;
        let event = &scheduling.message.event;

        // The address set: the account's primary identity, its configured aliases, and the
        // addresses this very message was delivered to. The third is what makes an invitation
        // to an alias work with no setup at all (§4 source 2).
        let mut addresses = self.account_address_set(&message.account).await;
        addresses.extend(scheduling.delivery_recipients.iter().cloned());

        let matched = matched_attendee(event, &addresses);
        let kind = crate::invitations::classify(&scheduling.message.method, matched.is_some())?;

        let zone = self.active_zone();
        // Without an instant there is no card worth drawing: no "when", no conflict window, no
        // preview. This is the case the Windows-TZID fix (engine G3) exists to prevent; an
        // Outlook invitation used to land here.
        let starts_at = resolve_instant_in(&event.start, &zone).ok()?;
        let ends_at = end_of(starts_at, event);

        let (conflicts, preview, stored_meeting) = self
            .conflicts_for(
                &message.account,
                event,
                kind,
                starts_at,
                ends_at,
                &addresses,
            )
            .await;

        // The calendar is the authority on which revision of a meeting is current, so the answer
        // to "is this mail still the invitation?" is the same lookup the tally and `my_response`
        // already ride on: no extra read. A `None` here means the calendar has nothing to
        // compare against, which is "we have not looked", never "this mail is current"; the card
        // then stays answerable, exactly as it does today.
        let kind = crate::invitations::supersede(kind, event.sequence, stored_meeting.as_ref());

        let (description_text, description_truncated) = description(event);
        let controls = self.account_rsvp_controls(&message.account).await;
        let delivery = self.account_delivery(&message.account).await;
        Some(InvitationCard {
            kind,
            organizer: organizer_line(event),
            summary: summary(event),
            location: location(event),
            description: description_text,
            description_truncated,
            starts_at: starts_at.to_string(),
            ends_at: ends_at.to_string(),
            all_day: event.is_all_day(),
            recurring: event.is_recurring(),
            // The calendar's copy wins for both, where there is one; see `conflicts_for`.
            // Answering must move *my* line and the tally together, or the card contradicts
            // itself: "You accepted", beside "1 yet to answer", where the one is you. The
            // mail is the fallback for a meeting the calendar has not synced (or a cold start
            // that has not looked yet), where it is still the best fact available.
            my_response: my_response(stored_meeting.as_ref().unwrap_or(event), &addresses),
            attendees: tally(stored_meeting.as_ref().unwrap_or(event)),
            conflict_count: conflicts.count(),
            conflicts_known: conflicts.is_known(),
            preview,
            // Gated on whether the answer can be **delivered**, not merely stored. The two
            // used to be the same test, which is how a plain CalDAV account came to offer
            // three buttons whose reply nobody would ever send (`docs/invitations.md`).
            can_respond: matches!(kind, InvitationKind::Rsvp) && delivery != Delivery::None,
            // On the iMIP route both controls are ours to honour rather than a transport's:
            // the note becomes a `COMMENT` property in the `REPLY` we build, and "tell the
            // organiser" literally decides whether we send the message. So an account that
            // could offer neither over CalDAV gains both: the same two controls Outlook
            // shows, on the transport that until now had none.
            can_comment: match delivery {
                Delivery::Server => controls.is_some_and(|c| c.comment),
                Delivery::ClientImip => true,
                Delivery::None => false,
            },
            can_choose_notify: match delivery {
                Delivery::Server => controls.is_some_and(|c| c.suppress_notification),
                Delivery::ClientImip => true,
                Delivery::None => false,
            },
        })
    }

    /// Reads the iTIP payload behind `message` through the engine facade.
    ///
    /// Cache-first on the raw source, so opening a message costs one fetch for its body and
    /// nothing extra for its invitation.
    pub(crate) async fn fetch_scheduling(
        &self,
        message: &MessageRef,
        original: &Message,
    ) -> Option<InboundScheduling> {
        let acct = self.account_handle(&message.account).await?;
        let provider = acct.providers.first()?;
        self.engine
            .message_scheduling(provider, &message.account, original)
            .await
            .ok()
            .flatten()
    }

    /// Every address that is *this account's own*: its primary identity plus its configured
    /// aliases.
    ///
    /// This is the **persisted** set, and therefore the one the calendar grid uses too: a grid
    /// has no message to read delivery headers from. The reading view widens it with the
    /// message's own delivery recipients.
    pub(crate) async fn account_address_set(&self, account: &AccountId) -> Vec<String> {
        let mut addresses = Vec::new();
        if let Some(identity) = self.account_identity(account).await {
            addresses.push(identity.email);
        }
        // Read from disk rather than caching: an alias list is edited rarely and read on two
        // cold paths (opening a message, rebuilding the calendar cache), both of which already do
        // store or network I/O: so one small TOML read is noise, and it can never be stale after
        // the user edits it.
        if let Some(path) = &self.prefs_path {
            addresses.extend(
                load_preferences(path)
                    .aliases_of(account.as_str())
                    .iter()
                    .cloned(),
            );
        }
        addresses
    }

    /// Forgets every alias recorded for `account`, on removal.
    ///
    /// Lives here beside [`Self::account_address_set`], the only reader of that list: the two are
    /// the same fact (which addresses are this account's own), and a set that outlived its
    /// account would silently answer "yes, that invitation is yours" for a re-added id.
    pub(crate) fn remove_account_aliases(&self, account: &str) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        let mut prefs = load_preferences(path);
        if prefs.remove_account_aliases(account) {
            let _ = mailcal_account::save_preferences(path, &prefs);
        }
    }

    /// Forgets `account`'s standing answer to the "shall we email the organiser ourselves?"
    /// prompt, on removal.
    ///
    /// Carried out for a sharper reason than the alias list: the stored choice can be
    /// [`ReplyFallback::Always`](mailcal_account::ReplyFallback::Always), which is a standing
    /// permission to **send mail as the user**. Inheriting that on a re-added id would mean the
    /// app quietly mails an organiser on behalf of an account the user had removed and set up
    /// again, having never been asked on this one.
    pub(crate) fn remove_reply_fallback(&self, account: &str) {
        let Some(path) = &self.prefs_path else {
            return;
        };
        let mut prefs = load_preferences(path);
        if prefs.remove_reply_fallback(account) {
            let _ = mailcal_account::save_preferences(path, &prefs);
        }
    }

    /// Counts what the meeting clashes with, and builds the one-day preview grid.
    ///
    /// One pass, so the number and the picture can never disagree: both come from the same diary
    /// read over the same window, and both apply the same declined-is-hidden rule. The picture
    /// carries one thing the count does not: the meeting itself, drawn as a proposed hold where
    /// no calendar holds it (`proposed_hold`), which is the block the count excludes by `UID`
    /// anyway.
    ///
    /// **Scoped to `account`.** The diary read covers the account the invitation arrived on, not
    /// every configured account: so on a multi-account setup the number can miss a clash sitting
    /// in another calendar, while the grid the user knows is unified. A recorded shortfall
    /// (`docs/invitations.md` → Known gaps), not an oversight: pooling the accounts also requires
    /// the preview to show whose calendar a block belongs to.
    ///
    /// **Returns `Unknown` rather than zero whenever it cannot answer**: a read failure, an
    /// unresolvable day, or a calendar the engine has not expanded over the meeting yet. Zero and
    /// "we have not looked" are different facts, and collapsing them ships the confident lie
    /// `docs/calendar.md` §4 exists to forbid: "Nothing else in your calendar then", stated over a
    /// calendar nobody has read. It is the same distinction `CalendarPage::is_materialized` draws
    /// for the grid, from the same window, and it is *not* hypothetical; opening an invitation
    /// before the first calendar sync (which the mail sync beats, so it is the common case on a
    /// cold start) hits it every time.
    async fn conflicts_for(
        &self,
        account: &AccountId,
        event: &Event,
        kind: InvitationKind,
        starts_at: UtcDateTime,
        ends_at: UtcDateTime,
        addresses: &[String],
    ) -> (Conflicts, TimeGrid, Option<Event>) {
        let zone = self.active_zone();
        let Ok(local) = to_local(starts_at, &zone) else {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        };
        // `LocalDateTime` carries no `date()`; build the civil date from its parts, the way the
        // grid solver does.
        let Ok(day) = CalendarDate::new(local.year(), local.month(), local.day()) else {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        };
        if !self.calendar_covers(day) {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        }
        let Some(window) = diary_window(day, &zone, starts_at, ends_at) else {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        };
        let Ok(rows) = self.engine.occurrences_in(account, window).await else {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        };
        let wanted: Vec<ProviderKey> = rows
            .iter()
            .map(|row| row.event.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let Ok(masters) = self.engine.events_by_keys(account, &wanted).await else {
            return (Conflicts::Unknown, TimeGrid::default(), None);
        };
        let by_key: HashMap<&str, &Event> = masters
            .iter()
            .map(|master| (master.id.key().as_str(), master))
            .collect();

        // Pair every occurrence row with the master that carries its content and this account's
        // answer to it. A row whose master is missing is a torn read between the two calls; skip
        // it rather than counting a clash with something we cannot name.
        let joined: Vec<(&OccurrenceRow, &Event, ResponseStatus)> = rows
            .iter()
            .filter_map(|row| {
                let master = *by_key.get(row.event.as_str())?;
                Some((row, master, diary_participation(master, addresses)))
            })
            .collect();

        let diary: Vec<DiaryEntry> = joined
            .iter()
            .map(|(row, master, response)| DiaryEntry {
                uid: master.uid.as_str().to_owned(),
                start: row.start,
                end: row.end,
                my_response: *response,
            })
            .collect();
        let conflicts = count_conflicts(event.uid.as_str(), starts_at, ends_at, &diary);

        // The meeting as the **calendar** holds it, which is the only copy that can change.
        // The invitation email is frozen at the moment it was sent, so a card built from it
        // alone would still read "you haven't answered" after you had, and, worse, would keep
        // counting you among the people yet to reply. Free: this join already ran for the
        // conflict count.
        let stored_meeting = joined
            .iter()
            .find(|(_, master, _)| master.uid.as_str() == event.uid.as_str())
            .map(|(_, master, _)| (*master).clone());

        // The preview draws the set the count describes, minus nothing: a declined event is
        // hidden here for exactly the reason it is hidden on the main grid (`docs/calendar.md`),
        // and the invitation's own tentative hold is *kept* so the user can see where the meeting
        // would land among their commitments.
        let mut occurrences: Vec<Occurrence> = joined
            .iter()
            .filter(|(_, _, response)| *response != ResponseStatus::Declined)
            .map(|(row, master, response)| Occurrence {
                account: account.as_str().to_owned(),
                event: master.id.key().as_str().to_owned(),
                calendar: master
                    .calendars
                    .iter()
                    .next()
                    .map(|id| id.key().as_str().to_owned())
                    .unwrap_or_default(),
                title: master.title.clone(),
                start: row.start,
                end: row.end,
                all_day: master.is_all_day(),
                // The preview is a picture of a day, not a calendar: it has no gestures at all,
                // so nothing here can be written, dragged, or asked about a series.
                can_write: false,
                can_move: false,
                occurrence_start: String::new(),
                participation: *response,
            })
            .collect();

        // …and the meeting itself, where no calendar holds it. A mailbox nothing files into leaves
        // the one block the card is about missing from its own picture, so the core draws it from
        // the mail; unanswered, therefore dotted, exactly as the stored copy would be. It changes
        // no count: `count_conflicts` skips the invitation's own `UID` either way.
        let mine = my_response(event, addresses);
        if proposed_hold(kind, stored_meeting.as_ref(), mine) {
            occurrences.push(Occurrence {
                account: account.as_str().to_owned(),
                // Nothing to point at: there is no stored event to key and no calendar to colour
                // it by. The preview carries no calendar list, so every block already draws in the
                // neutral swatch, and none of them is tappable on any client.
                event: String::new(),
                calendar: String::new(),
                // The card's own sanitiser, because this title arrives from the mail rather than
                // through the calendar: the same string the card shows two rows above the grid.
                title: summary(event),
                start: starts_at,
                end: ends_at,
                all_day: event.is_all_day(),
                // Same as the blocks above: the preview is a picture of a day, not a calendar, so
                // nothing in it can be written, dragged, or asked about a series.
                can_write: false,
                can_move: false,
                occurrence_start: String::new(),
                participation: mine,
            });
        }
        (
            Conflicts::Known(conflicts),
            grid::build(&[day], &occurrences, &zone),
            stored_meeting,
        )
    }

    /// What `account`'s calendar provider can do with an RSVP, or `None` if it cannot answer.
    ///
    /// Gates the buttons the way `can_write` already gates event editing: absent with an
    /// explanation, never present and disabled. And the two controls *around* the answer are
    /// gated separately, because they are not universal: a note has nowhere to go on CalDAV
    /// or JMAP, and neither transport can be told to keep the organiser out of it, since its
    /// server sends the reply the moment the status changes. Offering either there would be a
    /// control that lies.
    pub(crate) async fn account_rsvp_controls(
        &self,
        account: &AccountId,
    ) -> Option<engine_api::RsvpControls> {
        self.account_handle(account)
            .await?
            .calendar_providers
            .first()?
            .connection_info()
            .capabilities
            .calendar_rsvp()
    }

    /// Which route an answer on `account` would take to the organiser.
    ///
    /// Reads **both** halves of the account, because the answer to "can this be answered?" is
    /// not a property of either one alone: the calendar transport says whether the answer can
    /// be stored and whether the server will schedule it, and the mail transport says whether
    /// we could send the iMIP message ourselves if it will not. An account with no calendar
    /// provider still reaches [`Delivery::ClientImip`]; see [`delivery`] for why that is the
    /// honest answer rather than an oversight.
    pub(crate) async fn account_delivery(&self, account: &AccountId) -> Delivery {
        let Some(acct) = self.account_handle(account).await else {
            return Delivery::None;
        };
        let calendar = acct
            .calendar_providers
            .first()
            .map(|provider| provider.connection_info().capabilities);
        let mail = acct
            .providers
            .first()
            .map(|provider| provider.connection_info().capabilities);
        delivery(
            calendar.is_some_and(|caps| caps.calendar_rsvp().is_some()),
            calendar.is_some_and(engine_api::Capabilities::calendar_scheduling),
            mail.is_some_and(engine_api::Capabilities::scheduling_submission),
        )
    }

    /// Whether the calendar has actually been expanded over `day`.
    ///
    /// The same question, from the same window, that `CalendarPage::is_materialized` answers for a
    /// grid page, asked here so the conflict count can say "we have not looked" instead of "zero".
    /// `None` (no sync yet) is the honest answer on a cold start, and it is the common one: mail
    /// syncs before calendars, so an invitation opened on launch reaches this first.
    fn calendar_covers(&self, day: CalendarDate) -> bool {
        let cache = self.calendar_cache.lock().expect("calendar cache poisoned");
        crate::calendar_cache::covers(cache.window, &[day])
    }
}

/// What the diary read could establish about the meeting's window.
///
/// A two-case enum rather than a bare `u32` because **zero and "unknown" are different answers**,
/// and every path in `conflicts_for` that cannot read the calendar used to return the first while
/// meaning the second. A client renders them as different sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Conflicts {
    /// The calendar was read: this many other commitments overlap.
    Known(u32),
    /// The calendar could not be read over this window; say so, do not print a count.
    Unknown,
}

impl Conflicts {
    /// The count, or zero when unknown. Only meaningful alongside [`Self::is_known`].
    pub(crate) const fn count(self) -> u32 {
        match self {
            Self::Known(count) => count,
            Self::Unknown => 0,
        }
    }

    /// Whether the count means anything.
    pub(crate) const fn is_known(self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// The meeting's end instant: its start plus its calendar duration.
///
/// The engine models an event as start + duration, and `UtcDateTime::checked_add` takes an
/// elapsed `core::time::Duration`; deliberately a different type from the calendar one, so the
/// conversion is explicit here rather than implied. A zero or overflowing duration collapses to
/// the start, which the half-open overlap test then treats as touching nothing.
pub(crate) fn end_of(starts_at: UtcDateTime, event: &Event) -> UtcDateTime {
    let calendar = event.duration;
    let elapsed = core::time::Duration::new(
        calendar
            .days()
            .saturating_mul(86_400)
            .saturating_add(calendar.seconds()),
        calendar.nanoseconds(),
    );
    starts_at.checked_add(elapsed).unwrap_or(starts_at)
}

/// The window to read the diary over: the meeting's **local day**, widened to cover the meeting
/// itself if it runs past that day.
///
/// The day is the right unit because it is exactly what the preview grid draws, so the number and
/// the picture describe the same set. Widening matters for a multi-day booking, whose end lies
/// outside its start day. The store matches occurrences by *overlap*, so a long event that began
/// yesterday is still returned without any extra slack.
fn diary_window(
    day: CalendarDate,
    zone: &TimeZoneId,
    starts_at: UtcDateTime,
    ends_at: UtcDateTime,
) -> Option<Horizon> {
    let bounds = day_bounds_utc(day, zone).ok()?;
    let from = bounds.start().min(starts_at);
    let to = bounds.end().max(ends_at);
    Horizon::new(from, to).ok()
}
