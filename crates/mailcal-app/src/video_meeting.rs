//! The video meeting the editor puts on an event or takes off (`docs/conferencing.md`, rules 9
//! to 13): which meetings a calendar offers, the Jitsi server they depend on, and a choice
//! turned into the engine's write.

use engine_api::{
    AccountId, Calendar, CalendarDateTime, ConferenceEdit, ConferenceService, Event, EventDraft,
    JitsiMeetings, MeetingKey, MeetingRequest, MeetingService, NewConference, Provider,
    UtcDateTime, resolve_instant_in,
};
use mailcal_account::{MeetingChange, MeetingChoice, current_meeting};
use mailcal_viewmodel::calendar::MeetingOption;
use ring::rand::{SecureRandom, SystemRandom};

use crate::{Account, App, calendar_cache::CachedCalendar};

/// The meetings `calendar`'s editor offers, in order: its own server's, then Jitsi while a Jitsi
/// server is set (rule 9). None where its account cannot write a meeting.
pub(crate) fn meeting_choices(calendar: &CachedCalendar, jitsi: bool) -> Vec<MeetingOption> {
    if !calendar.writes_meetings {
        return Vec::new();
    }
    calendar
        .server_meetings
        .iter()
        .cloned()
        .map(MeetingChoice::Server)
        // A service this build has no name for is no choice a person could read.
        .filter(|choice| MeetingChoice::from_id(&choice.id()).is_some())
        .chain(jitsi.then_some(MeetingChoice::Jitsi))
        .map(|choice| MeetingOption {
            id: choice.id(),
            name: choice.name(),
        })
        .collect()
}

/// A Jitsi server the person typed that the core will not store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JitsiServerRefused;

impl<P: Provider> App<P> {
    /// Whether `account` can put a video meeting on an event: its calendar is written, and its
    /// provider writes meetings.
    pub(crate) fn account_writes_meetings(account: &Account<P>) -> bool {
        Self::account_can_write(account)
            && account
                .calendar_provider
                .as_ref()
                .is_some_and(|provider| provider.connection_info().capabilities.conference_writes())
    }

    /// The person's Jitsi server, or `None` until they set one (rule 13).
    #[must_use]
    pub fn jitsi_server(&self) -> Option<String> {
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .jitsi_server()
            .map(str::to_owned)
    }

    /// Stores the person's Jitsi server, or forgets it for `None` or blank text, and redraws the
    /// calendar rows whose choices it changes.
    ///
    /// # Errors
    ///
    /// [`JitsiServerRefused`] for anything but an `https` address without a query or fragment,
    /// which then changes nothing.
    pub fn set_jitsi_server(&self, server: Option<&str>) -> Result<(), JitsiServerRefused> {
        let server = match server.map(str::trim).filter(|server| !server.is_empty()) {
            Some(server) => Some(
                JitsiMeetings::new(server)
                    .map_err(|_| JitsiServerRefused)?
                    .server()
                    .to_owned(),
            ),
            None => None,
        };
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .set_jitsi_server(server);
        self.calendar.resignal();
        Ok(())
    }

    /// The meeting `change` puts on an event in a calendar whose server makes `calendar`'s
    /// meetings, from `start` to `end`, titled `title`; `None` when it takes one off. A server-made
    /// one is asked of the server; a Jitsi one is made here, sending nothing (rule 12).
    ///
    /// # Errors
    ///
    /// The calendar does not offer `choice`, the Jitsi server is unset, or the times cannot be
    /// placed.
    pub(crate) async fn new_conference(
        &self,
        calendar: &[ConferenceService],
        change: &MeetingChange,
        title: &str,
        start: UtcDateTime,
        end: UtcDateTime,
    ) -> Result<Option<NewConference>, String> {
        let MeetingChange::Set { choice, words } = change else {
            return Ok(None);
        };
        match choice {
            MeetingChoice::Server(service) => {
                if !calendar.contains(service) {
                    return Err("this calendar's server makes no such meeting".to_owned());
                }
                Ok(Some(NewConference::Native(service.clone())))
            }
            MeetingChoice::Jitsi => {
                let server = self
                    .jitsi_server()
                    .ok_or_else(|| "no Jitsi server is set".to_owned())?;
                let jitsi = JitsiMeetings::new(&server).map_err(|err| err.to_string())?;
                let request = MeetingRequest {
                    key: mint_key()?,
                    title: title.to_owned(),
                    start,
                    end,
                };
                let conference = jitsi
                    .create_meeting(&request)
                    .await
                    .map_err(|err| err.to_string())?;
                Ok(Some(NewConference::attach(conference, (**words).clone())))
            }
        }
    }

    /// The engine's edit for `change` on `stored`, whose calendar's server makes `calendar`'s
    /// meetings: the event's current meeting taken off for None, replaced for a choice
    /// (rule 11).
    ///
    /// # Errors
    ///
    /// None for an event with no meeting, or [`Self::new_conference`]'s refusals.
    pub(crate) async fn conference_edit(
        &self,
        stored: &Event,
        calendar: &[ConferenceService],
        change: &MeetingChange,
    ) -> Result<ConferenceEdit, String> {
        let current = current_meeting(stored).map(|conference| conference.key.clone());
        let start = self.instant(&stored.start)?;
        let span = core::time::Duration::from_secs(
            stored.duration.days() * 86_400 + stored.duration.seconds(),
        );
        let end = start
            .checked_add(span)
            .ok_or_else(|| "the event ends past representable time".to_owned())?;
        match self
            .new_conference(calendar, change, &stored.title, start, end)
            .await?
        {
            Some(new) => Ok(ConferenceEdit::Set {
                replacing: current,
                new,
            }),
            None => current
                .map(ConferenceEdit::Remove)
                .ok_or_else(|| "the event has no meeting to take off".to_owned()),
        }
    }

    /// `draft` with the meeting `change` puts on it, or `None`, logged, when `calendar` cannot
    /// have that meeting or it could not be made: the event is then not saved, since saving it
    /// without the meeting the person chose would say it had one.
    pub(crate) async fn with_meeting(
        &self,
        account: &AccountId,
        calendar: &Calendar,
        draft: EventDraft,
        change: &MeetingChange,
    ) -> Option<EventDraft> {
        let made = async {
            self.writes_meetings_in(account, calendar).await?;
            let start = self.instant(&draft.start)?;
            let end = self.instant(&draft.end)?;
            self.new_conference(
                &calendar.native_conferences,
                change,
                &draft.summary,
                start,
                end,
            )
            .await
        }
        .await;
        match made {
            Ok(Some(new)) => Some(draft.conference(new)),
            Ok(None) => Some(draft),
            Err(err) => {
                log::warn!("the new event's video meeting was not made: {err}");
                None
            }
        }
    }

    /// The engine's edit putting `change` on `stored`, in the calendar it belongs to.
    ///
    /// # Errors
    ///
    /// The calendar cannot have the meeting, or [`Self::conference_edit`]'s refusals.
    pub(crate) async fn meeting_edit(
        &self,
        account: &AccountId,
        stored: &Event,
        change: &MeetingChange,
    ) -> Result<ConferenceEdit, String> {
        let calendars = self.engine.calendars(account).await.unwrap_or_default();
        let calendar = stored
            .calendars
            .iter()
            .next()
            .and_then(|id| calendars.iter().find(|calendar| &calendar.id == id))
            .ok_or_else(|| "the event's calendar is not listed".to_owned())?;
        self.writes_meetings_in(account, calendar).await?;
        self.conference_edit(stored, &calendar.native_conferences, change)
            .await
    }

    /// Whether a meeting can be put on an event in `calendar`.
    async fn writes_meetings_in(
        &self,
        account: &AccountId,
        calendar: &Calendar,
    ) -> Result<(), String> {
        let writes = self
            .account_handle(account)
            .await
            .is_some_and(|account| Self::account_writes_meetings(&account));
        if writes && calendar.access.may_write {
            Ok(())
        } else {
            Err("this calendar takes no video meeting".to_owned())
        }
    }

    /// `value` as an instant, a zoneless one placed in the app's zone.
    pub(crate) fn instant(&self, value: &CalendarDateTime) -> Result<UtcDateTime, String> {
        resolve_instant_in(value, &self.active_zone()).map_err(|err| err.to_string())
    }
}

/// A fresh meeting key: 128 bits from the OS's secure random source, in lowercase base32, since
/// anyone who has it can join a meeting whose link is the key.
fn mint_key() -> Result<MeetingKey, String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "the OS gave no random bytes".to_owned())?;
    MeetingKey::new(base32(&bytes)).map_err(|err| err.to_string())
}

/// `bytes` in RFC 4648 base32, lowercase and unpadded: 26 characters for 16 bytes.
fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

#[cfg(test)]
#[path = "video_meeting_tests.rs"]
mod tests;
