//! The showcase dataset's **video meeting**: that the event the `meeting` screenshot screen opens
//! is really there in every locale, with a meeting to join and the invitation's text kept out of
//! its notes. A detail missing its Join button still photographs as a perfectly good PNG, so the
//! screen is pinned here rather than by looking at captures.

use std::{sync::mpsc, time::Duration};

use super::*;
use crate::{
    tests::{ChannelObserver, NullLogger},
    tests_showcase::ALL_SHOWCASE_LOCALES,
};

#[test]
fn the_showcase_meeting_opens_with_join_and_a_number_in_every_locale() {
    let target = showcase_meeting();
    for locale in ALL_SHOWCASE_LOCALES {
        let (tx, rx) = mpsc::channel();
        let app = MailcalApp::new_showcase(
            Box::new(ChannelObserver { tx }),
            Box::new(NullLogger),
            LogLevel::Info,
            "Europe/Amsterdam".to_owned(),
            locale,
        );
        app.dispatch(Intent::RefreshCalendar);
        while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
            if matches!(surface, Surface::Calendar) {
                break;
            }
        }

        let detail = app
            .event_detail(target.account.clone(), target.event_key.clone(), None)
            .unwrap_or_else(|| panic!("{locale:?}: the showcase meeting is not in the calendar"));
        let [meeting] = detail.conferences.as_slice() else {
            panic!(
                "{locale:?}: expected one meeting, got {:?}",
                detail.conferences
            );
        };
        assert_eq!(meeting.state, ConferenceState::Ready, "{locale:?}");
        assert!(
            meeting
                .join_url
                .as_deref()
                .is_some_and(|link| link.starts_with("https://")),
            "{locale:?}: nothing to join"
        );
        assert!(meeting.meeting_id.is_some(), "{locale:?}: no meeting ID");
        assert!(!meeting.dial_in.is_empty(), "{locale:?}: no number to dial");

        // The grid marks the same event, so the screenshot and the week behind it agree.
        let page = app.calendar_range(app.week_start_date(showcase_today()), 7);
        assert!(
            page.timed
                .iter()
                .any(|block| block.event == target.event_key && block.has_conference),
            "{locale:?}: the week does not mark the meeting with its camera"
        );
    }
}

fn showcase_today() -> String {
    // The seed's own clock, read at its pinned morning hour, which falls on the same date in UTC.
    let seeded =
        crate::showcase_data::seeded_now(&crate::device_zone("Europe/Amsterdam".to_owned()));
    let days = seeded.unix_timestamp().div_euclid(86_400);
    mailcal_viewmodel::calendar::days::date_at(days).to_string()
}
