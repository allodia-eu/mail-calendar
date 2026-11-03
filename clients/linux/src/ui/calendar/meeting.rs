//! An event's video meetings: the decisions behind the detail's meeting rows and the camera on
//! every event, with no widget in them so a plain test can drive them (`docs/conferencing.md`).
//! The rows themselves are `conference.rs`.
//!
//! Which meetings exist, which link Join opens and which numbers are dialled is the core's
//! (`EventDetail::conferences`): `join_url` and a dial-in's `uri` have already passed the link
//! policy, so they are opened as they are.

use mailcal_bindings::{ConferenceState, EventConference};

use crate::l10n;

/// The meeting's heading: the service's own name, a brand the same in every language, or the
/// catalog's words for a video meeting when nothing names one.
pub(super) fn title(conference: &EventConference) -> &str {
    conference
        .service
        .as_deref()
        .filter(|service| !service.is_empty())
        .unwrap_or_else(|| l10n::event_video_meeting())
}

/// What the detail says in place of Join while the server is still making the meeting, or after it
/// could not; `None` for a meeting that is ready.
pub(super) fn status(conference: &EventConference) -> Option<&'static str> {
    match conference.state {
        ConferenceState::Ready => None,
        ConferenceState::Preparing => Some(l10n::event_meeting_preparing()),
        ConferenceState::Failed => Some(l10n::event_meeting_failed()),
    }
}

/// `value`, unless the server sent nothing there: a row with no value is left out.
pub(super) fn shown(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

/// An event's spoken label with "video meeting" after it when it has one: the camera is painted
/// or hidden from assistive technology, so the label says what it shows.
pub(super) fn spoken(label: String, has_conference: bool) -> String {
    if has_conference {
        format!("{label}, {}", l10n::event_video_meeting())
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{ConferenceState, EventConference};

    use super::{shown, spoken, status, title};
    use crate::l10n;

    fn meeting(state: ConferenceState) -> EventConference {
        EventConference {
            service: Some("Microsoft Teams".to_owned()),
            join_url: None,
            meeting_id: Some("123 456".to_owned()),
            passcode: None,
            dial_in: Vec::new(),
            notes: None,
            state,
        }
    }

    #[test]
    fn a_meeting_is_headed_by_its_service_and_otherwise_by_the_catalogs_words() {
        let mut named = meeting(ConferenceState::Ready);
        assert_eq!(title(&named), "Microsoft Teams");
        named.service = None;
        assert_eq!(title(&named), l10n::event_video_meeting());
        named.service = Some(String::new());
        assert_eq!(title(&named), l10n::event_video_meeting());
    }

    #[test]
    fn a_meeting_still_being_made_says_so_instead_of_offering_join() {
        assert_eq!(
            status(&meeting(ConferenceState::Preparing)),
            Some(l10n::event_meeting_preparing())
        );
        assert_eq!(
            status(&meeting(ConferenceState::Failed)),
            Some(l10n::event_meeting_failed())
        );
        assert_eq!(status(&meeting(ConferenceState::Ready)), None);
    }

    #[test]
    fn an_empty_value_is_left_out() {
        assert_eq!(shown(Some("")), None);
        assert_eq!(shown(None), None);
        assert_eq!(shown(Some("123")), Some("123"));
    }

    #[test]
    fn an_event_with_a_meeting_says_so_out_loud() {
        assert_eq!(spoken("Sync".to_owned(), false), "Sync");
        assert_eq!(
            spoken("Sync".to_owned(), true),
            format!("Sync, {}", l10n::event_video_meeting())
        );
    }
}
