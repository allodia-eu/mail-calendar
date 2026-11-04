//! The video meeting the editor puts on an event or takes off (`docs/conferencing.md`, rules 9
//! to 13): which meetings a calendar offers, what a write asks for, and what the editor opens
//! showing.

use engine_api::{Conference, ConferenceOrigin, ConferenceService, ConferenceText, Event};
use mailcal_viewmodel::calendar::meeting::is_shown;

use crate::event_conference::service_name;

/// A meeting the editor may put on an event. A calendar row lists the ones it offers, and a
/// write names one back by its [`id`](Self::id).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MeetingChoice {
    /// A meeting the calendar's own server makes on this service.
    Server(ConferenceService),
    /// A Jitsi meeting on the person's own Jitsi server.
    Jitsi,
}

/// Before a server-made service's token in a choice's id.
const SERVER: &str = "server:";
/// A Jitsi choice's id.
const JITSI: &str = "jitsi";

impl MeetingChoice {
    /// The id a client sends back to name this choice: `server:<service>` or `jitsi`.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Server(service) => format!("{SERVER}{}", service.as_str()),
            Self::Jitsi => JITSI.to_owned(),
        }
    }

    /// The choice `id` names, or `None` for one no row offers.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        if id == JITSI {
            return Some(Self::Jitsi);
        }
        let service = ConferenceService::from_wire(id.strip_prefix(SERVER)?);
        service_name(&service).map(|_| Self::Server(service))
    }

    /// The service's name, a brand name the same in every language.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Server(service) => service_name(service).unwrap_or_default().to_owned(),
            Self::Jitsi => "Jitsi Meet".to_owned(),
        }
    }

    /// Whether `conference` is a meeting this choice makes: one the server made on its service,
    /// or a Jitsi meeting put on the event from elsewhere.
    fn made(&self, conference: &Conference) -> bool {
        match self {
            Self::Server(service) => {
                matches!(conference.origin, ConferenceOrigin::Native { .. })
                    && conference.service.as_ref() == Some(service)
            }
            Self::Jitsi => {
                conference.origin == ConferenceOrigin::Attached
                    && conference.service == Some(ConferenceService::Jitsi)
            }
        }
    }
}

/// What a write does to an event's meeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeetingChange {
    /// Take the event's meeting off (rule 11's None).
    Remove,
    /// Put a meeting on the event, replacing the one it has.
    Set {
        /// The meeting to put on.
        choice: MeetingChoice,
        /// The words of the block the notes carry for a meeting made elsewhere, in the event's
        /// language: the client's, since the core has no locale. Boxed, so an intent carrying no
        /// meeting does not carry room for six strings.
        words: Box<ConferenceText>,
    },
}

/// The meeting an editor opens showing (rule 11): the event's first shown meeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMeeting {
    /// The service's name, or `None` when nothing names it; a client then shows its own words
    /// for a video meeting.
    pub name: Option<String>,
    /// The choice that made it, when one did; `None` for a meeting no choice makes (a Zoom link
    /// from an invitation), which can be taken off but not chosen again.
    pub choice: Option<MeetingChoice>,
}

/// The meeting the editor works on: the event's first shown one.
#[must_use]
pub fn current_meeting(event: &Event) -> Option<&Conference> {
    event
        .conferences
        .iter()
        .find(|conference| is_shown(conference))
}

/// What the editor opens showing for `event`, or `None` when it has no shown meeting.
#[must_use]
pub fn editor_meeting(event: &Event) -> Option<EditorMeeting> {
    let conference = current_meeting(event)?;
    let choice = conference.service.clone().and_then(|service| {
        [MeetingChoice::Server(service), MeetingChoice::Jitsi]
            .into_iter()
            .find(|choice| choice.made(conference))
    });
    Some(EditorMeeting {
        name: conference
            .service
            .as_ref()
            .and_then(service_name)
            .map(str::to_owned),
        choice,
    })
}

#[cfg(test)]
#[path = "event_meeting_tests.rs"]
mod tests;
