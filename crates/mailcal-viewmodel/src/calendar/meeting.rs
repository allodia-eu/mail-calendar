//! Which of an event's conferences a person can act on: the one rule behind the detail view's
//! meetings and the grid's camera glyph, so the two cannot disagree (`docs/conferencing.md`).

use engine_api::{Conference, ConferenceOrigin, EntryKind, Event, NativeState, UriScheme};

/// Whether `conference` is shown: it can be joined over `https`, dialled, or is a meeting the
/// calendar server is still making or could not make. A meeting whose only way in is a plain
/// `http` link or a SIP address offers nothing a client opens.
#[must_use]
pub fn is_shown(conference: &Conference) -> bool {
    let made_by_server_and_unready = matches!(
        conference.origin,
        ConferenceOrigin::Native {
            state: NativeState::Pending | NativeState::Failed,
            ..
        }
    );
    made_by_server_and_unready
        || conference.entry_points.iter().any(|entry| {
            (entry.kind == EntryKind::Video && entry.uri.scheme() == UriScheme::Https)
                || entry.uri.scheme() == UriScheme::Tel
        })
}

/// Whether `event` has a meeting [`is_shown`]: what the grid, the month and the agenda mark
/// with a camera glyph.
#[must_use]
pub fn has_conference(event: &Event) -> bool {
    event.conferences.iter().any(is_shown)
}

#[cfg(test)]
mod tests {
    use engine_api::{ConferenceKey, ConferenceUri, EntryPoint};

    use super::*;

    fn conference(origin: ConferenceOrigin, uris: &[&str]) -> Conference {
        Conference {
            key: ConferenceKey::new("k"),
            service: None,
            origin,
            entry_points: uris
                .iter()
                .map(|uri| EntryPoint::at(ConferenceUri::parse(uri).unwrap()))
                .collect(),
            label: None,
            meeting_id: None,
            notes: None,
        }
    }

    #[test]
    fn a_meeting_is_shown_when_it_can_be_joined_dialled_or_is_being_made() {
        let attached = ConferenceOrigin::Attached;
        assert!(is_shown(&conference(attached, &["https://zoom.us/j/1"])));
        assert!(is_shown(&conference(attached, &["tel:+3120"])));
        assert!(!is_shown(&conference(
            attached,
            &["http://meet.example.com/a"]
        )));
        assert!(!is_shown(&conference(attached, &["sip:room@example.com"])));
        let pending = ConferenceOrigin::Native {
            removable: false,
            state: NativeState::Pending,
        };
        assert!(is_shown(&conference(pending, &[])));
        let ready = ConferenceOrigin::Native {
            removable: false,
            state: NativeState::Ready,
        };
        assert!(!is_shown(&conference(ready, &[])));
    }
}
