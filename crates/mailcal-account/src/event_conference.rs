//! The meetings an event can be joined at, as the event detail shows them: the read half of
//! `docs/conferencing.md`.
//!
//! The engine reads every protocol's conference fields, and the links an invitation leaves in
//! a location or the description, into [`Conference`]s. This flattens each into what a person
//! acts on: a Join link, the codes to type, the numbers to dial.

use engine_core::calendar::{
    Conference, ConferenceOrigin, ConferenceService, EntryKind, EntryPoint, Event, NativeState,
    UriScheme,
};
use mailcal_viewmodel::calendar::meeting::is_shown;

/// Where a meeting the calendar server makes stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConferenceState {
    /// It can be joined, or its numbers dialled.
    Ready,
    /// The server was asked for it and has not made it yet: there is nothing to join.
    Preparing,
    /// The server was asked for it and could not make it.
    Failed,
}

/// A number to dial into a meeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialIn {
    /// The number as the server wrote it (`+1 425-555-0100`), for showing.
    pub number: String,
    /// The `tel:` URI that dials it.
    pub uri: String,
    /// The PIN or access code to type once connected, if the server gave one.
    pub pin: Option<String>,
}

/// One meeting on an event, as the detail view shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventConference {
    /// The service's name ("Microsoft Teams", "Zoom"), or `None` when neither the server nor
    /// the link names one; a client then shows its own generic words. A brand name, the same
    /// in every language.
    pub service: Option<String>,
    /// The `https` link a person joins at, opened through the OS, which hands it to the
    /// service's own app where one is installed. `None` while the meeting is being prepared,
    /// when it has only numbers, or when its link is not `https`.
    pub join_url: Option<String>,
    /// The ID a person types to join from an app or a phone.
    pub meeting_id: Option<String>,
    /// The passcode the join link asks for.
    pub passcode: Option<String>,
    /// The numbers to dial in on, in the server's order.
    pub dial_in: Vec<DialIn>,
    /// The server's or service's instructions for it, as plain text.
    pub notes: Option<String>,
    /// Where it stands.
    pub state: ConferenceState,
}

/// Every meeting on `event` that can be joined, dialled or is on its way, in the event's own
/// order (the calendar server's own first).
#[must_use]
pub fn event_conferences(event: &Event) -> Vec<EventConference> {
    event.conferences.iter().filter_map(project).collect()
}

fn project(conference: &Conference) -> Option<EventConference> {
    if !is_shown(conference) {
        return None;
    }
    let state = match conference.origin {
        ConferenceOrigin::Native {
            state: NativeState::Pending,
            ..
        } => ConferenceState::Preparing,
        ConferenceOrigin::Native {
            state: NativeState::Failed,
            ..
        } => ConferenceState::Failed,
        _ => ConferenceState::Ready,
    };
    let video = conference
        .entry_points
        .iter()
        .find(|entry| entry.kind == EntryKind::Video);
    let join_url = video
        .filter(|entry| entry.uri.scheme() == UriScheme::Https)
        .map(|entry| entry.uri.as_str().to_owned());
    let dial_in: Vec<DialIn> = conference
        .entry_points
        .iter()
        .filter(|entry| entry.uri.scheme() == UriScheme::Tel)
        .map(dial_in)
        .collect();
    Some(EventConference {
        service: conference
            .service
            .as_ref()
            .and_then(service_name)
            .map(str::to_owned),
        join_url,
        meeting_id: conference
            .meeting_id
            .clone()
            .or_else(|| video.and_then(|entry| entry.access.meeting_code.clone())),
        passcode: video.and_then(|entry| {
            entry
                .access
                .passcode
                .clone()
                .or_else(|| entry.access.password.clone())
        }),
        dial_in,
        notes: conference.notes.clone(),
        state,
    })
}

fn dial_in(entry: &EntryPoint) -> DialIn {
    let uri = entry.uri.as_str();
    DialIn {
        number: entry
            .label
            .clone()
            .unwrap_or_else(|| uri.trim_start_matches("tel:").to_owned()),
        uri: uri.to_owned(),
        pin: entry
            .access
            .pin
            .clone()
            .or_else(|| entry.access.access_code.clone())
            .or_else(|| entry.access.passcode.clone()),
    }
}

/// The name a service goes by, or `None` for one this build does not know.
fn service_name(service: &ConferenceService) -> Option<&'static str> {
    Some(match service {
        ConferenceService::Teams => "Microsoft Teams",
        ConferenceService::SkypeForBusiness => "Skype for Business",
        ConferenceService::Skype => "Skype",
        ConferenceService::GoogleMeet => "Google Meet",
        ConferenceService::Zoom => "Zoom",
        ConferenceService::Webex => "Webex",
        ConferenceService::Jitsi => "Jitsi Meet",
        ConferenceService::NextcloudTalk => "Nextcloud Talk",
        ConferenceService::Whereby => "Whereby",
        ConferenceService::ProtonMeet => "Proton Meet",
        ConferenceService::GoTo => "GoTo Meeting",
        ConferenceService::Other(_) => return None,
    })
}

/// Whether `location` says nothing but the join link of one of `conferences`, which the
/// detail already offers as Join, so showing it as a place would repeat it.
#[must_use]
pub(crate) fn is_join_link(location: &str, event: &Event) -> bool {
    let Ok(uri) = engine_core::calendar::ConferenceUri::parse(location) else {
        return false;
    };
    event.conferences.iter().any(|conference| {
        conference
            .entry_points
            .iter()
            .any(|entry| entry.uri.same_meeting(&uri))
    })
}

#[cfg(test)]
#[path = "event_conference_tests.rs"]
mod tests;
