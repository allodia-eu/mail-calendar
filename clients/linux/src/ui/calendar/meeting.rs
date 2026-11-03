//! An event's video meetings: the decisions behind the detail's meeting rows and the camera on
//! every event, with no widget in them so a plain test can drive them (`docs/conferencing.md`).
//! The rows themselves are `conference.rs`.
//!
//! Which meetings exist, which link Join opens and which numbers are dialled is the core's
//! (`EventDetail::conferences`). Join and a dial-in number are the two places a click leaves the
//! app, so what each will open is decided here.

use mailcal_bindings::{ConferenceDialIn, ConferenceState, EventConference};

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

/// The link Join opens and Copy link copies, as the core sent it, or `None` when there is nothing
/// to join: a meeting the server has not made, no link, or one the shared link policy
/// (`may_open`, `should_open_external_link`) or the `https` rule refuses.
pub(super) fn join_link(
    conference: &EventConference,
    may_open: impl Fn(&str) -> bool,
) -> Option<&str> {
    if conference.state != ConferenceState::Ready {
        return None;
    }
    let link = conference.join_url.as_deref()?;
    (has_scheme(link, "https") && may_open(link)).then_some(link)
}

/// The `tel:` URI a dial-in row opens. The core builds it from a validated number, and the shared
/// link policy admits web and mail links only, so the scheme is checked here.
pub(super) fn dial_in_link(number: &ConferenceDialIn) -> Option<&str> {
    has_scheme(&number.uri, "tel").then_some(number.uri.as_str())
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

fn has_scheme(link: &str, scheme: &str) -> bool {
    link.split_once(':')
        .is_some_and(|(found, _)| found.eq_ignore_ascii_case(scheme))
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{ConferenceDialIn, ConferenceState, EventConference};

    use super::{dial_in_link, join_link, shown, spoken, status, title};
    use crate::l10n;

    const TEAMS: &str = "https://teams.microsoft.com/meet/123?p=x";

    fn meeting(join_url: Option<&str>, state: ConferenceState) -> EventConference {
        EventConference {
            service: Some("Microsoft Teams".to_owned()),
            join_url: join_url.map(str::to_owned),
            meeting_id: Some("123 456".to_owned()),
            passcode: None,
            dial_in: Vec::new(),
            notes: None,
            state,
        }
    }

    fn any(_: &str) -> bool {
        true
    }

    #[test]
    fn a_meeting_is_headed_by_its_service_and_otherwise_by_the_catalogs_words() {
        let mut named = meeting(Some(TEAMS), ConferenceState::Ready);
        assert_eq!(title(&named), "Microsoft Teams");
        named.service = None;
        assert_eq!(title(&named), l10n::event_video_meeting());
        named.service = Some(String::new());
        assert_eq!(title(&named), l10n::event_video_meeting());
    }

    #[test]
    fn join_opens_the_cores_web_link_and_nothing_else() {
        assert_eq!(
            join_link(&meeting(Some(TEAMS), ConferenceState::Ready), any),
            Some(TEAMS)
        );
        assert_eq!(join_link(&meeting(None, ConferenceState::Ready), any), None);
        assert_eq!(
            join_link(
                &meeting(Some("javascript:alert(1)"), ConferenceState::Ready),
                any
            ),
            None
        );
        assert_eq!(
            join_link(
                &meeting(
                    Some("http://teams.microsoft.com/meet/1"),
                    ConferenceState::Ready
                ),
                any
            ),
            None,
            "plain http is not a join link"
        );
        assert_eq!(
            join_link(&meeting(Some(TEAMS), ConferenceState::Ready), |_| false),
            None,
            "the gate refused it"
        );
        assert!(mailcal_bindings::should_open_external_link(
            TEAMS.to_owned()
        ));
    }

    #[test]
    fn a_meeting_still_being_made_says_so_instead_of_offering_join() {
        let preparing = meeting(Some(TEAMS), ConferenceState::Preparing);
        assert_eq!(join_link(&preparing, any), None);
        assert_eq!(status(&preparing), Some(l10n::event_meeting_preparing()));
        assert_eq!(
            status(&meeting(None, ConferenceState::Failed)),
            Some(l10n::event_meeting_failed())
        );
        assert_eq!(status(&meeting(Some(TEAMS), ConferenceState::Ready)), None);
    }

    #[test]
    fn a_dial_in_row_opens_only_a_telephone_number() {
        let number = ConferenceDialIn {
            number: "+31 20 555 0100".to_owned(),
            uri: "tel:+31205550100".to_owned(),
            pin: Some("990#".to_owned()),
        };
        assert_eq!(dial_in_link(&number), Some("tel:+31205550100"));
        let web = ConferenceDialIn {
            uri: "https://example.com/".to_owned(),
            ..number
        };
        assert_eq!(dial_in_link(&web), None);
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
