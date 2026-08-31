//! The showcase dataset's **meeting invitation**: that opening it really produces a card with a
//! response to give and a readable day under it, that answering it moves, and that every locale
//! translates it.
//!
//! Split from `tests_showcase.rs` for the 500-line limit, and because these are the one part of
//! the dataset whose failure mode is invisible to a screenshot: a card missing its buttons, or
//! drawn over a calendar nobody read, still photographs as a perfectly good PNG.

use std::{sync::mpsc, time::Duration};

use super::*;
use crate::{
    tests::{ChannelObserver, NullLogger},
    tests_showcase::ALL_SHOWCASE_LOCALES,
};

/// Opens the showcase's designated invitation and returns the card the reading view built.
///
/// The whole point of the `invitation` screenshot screen, exercised end to end: sync the seeded
/// mail, open one message, and read the card off the FFI record; through the same loop a client
/// goes through, over the same in-memory dataset a capture runs on.
fn showcase_invitation_card(app: &MailcalApp, rx: &mpsc::Receiver<Surface>) -> InvitationCard {
    let target = showcase_invitation();
    app.dispatch(Intent::OpenMessage {
        account: target.account,
        key: target.message_key.clone(),
    });
    while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(surface, Surface::Reading) {
            let reading = app.reading_view();
            if reading.key == target.message_key {
                return reading
                    .invitation
                    .expect("the seeded invitation message carries a card");
            }
        }
    }
    panic!("the invitation never opened");
}

#[test]
fn the_showcase_invitation_offers_a_response_over_a_readable_day() {
    // The `invitation` screenshot screen is only worth capturing if all four of these hold at
    // once, and each fails silently: as a card with a missing row, which no size floor or
    // pixel assertion in `scripts/dev/showcase.sh` can see. So they are pinned here instead:
    //
    //  * the mail really is an iTIP REQUEST addressed to this account (`kind == Rsvp`),
    //  * the account's calendar provider can answer, so the buttons are drawn at all,
    //  * the calendar was actually read over the meeting's day, which is what makes every client
    //    open its day preview rather than collapse it, and
    //  * that day has a real commitment on it, so the conflict line reads as a clash rather than
    //    "nothing else in your calendar then".
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let card = showcase_invitation_card(&app, &rx);
    assert!(matches!(card.kind, InvitationKind::Rsvp), "an RSVP is owed");
    assert!(card.can_respond, "the buttons must be drawn, not explained");
    assert!(
        matches!(card.my_response, ResponseStatus::NeedsAction),
        "the seeded hold is unanswered, so the screenshot shows the choice"
    );
    assert!(
        card.conflicts_known,
        "the calendar must be read at boot, or every client collapses the day preview"
    );
    assert_eq!(
        card.conflict_count, 2,
        "the meeting is seeded inside Thursday's board meeting, on a day the team offsite \
         already spans: so the card has a real clash to report, not a free afternoon"
    );
    assert!(
        !card.preview.timed.is_empty(),
        "the day preview draws that clash rather than only stating it"
    );
    // The seeded roster, so the card's counts line has something to say in every bucket.
    assert_eq!(card.attendees.total, 4);
    assert_eq!(card.attendees.accepted, 2);
    assert_eq!(card.attendees.tentative, 1);
    assert_eq!(card.attendees.needs_action, 1);
    // The details rows: an organiser, a room, and notes that fit without being cut.
    assert!(card.organizer.contains("sofia.ruiz@northwind.example"));
    assert!(
        !card.location.is_empty(),
        "the Where row has something in it"
    );
    assert!(!card.description.is_empty() && !card.description_truncated);
}

#[test]
fn answering_the_showcase_invitation_moves_the_card() {
    // The buttons are shown because the showcase provider advertises an RSVP capability: so it
    // has to honour one. A provider that advertised and then refused would put three live-looking
    // buttons in front of whoever is taking the screenshots, and fail on the first tap.
    //
    // This also pins the *reconcile* half: the answer lands on the provider's copy, and the card
    // reads its answer off the calendar rather than off the frozen mail. A `sync_events` that
    // reported "nothing new" after the write (as it did for the mailbox scope) would leave the
    // card saying "you haven't answered" one dispatch after the user had.
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_showcase(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let target = showcase_invitation();
    let before = showcase_invitation_card(&app, &rx);
    assert!(matches!(before.my_response, ResponseStatus::NeedsAction));

    app.dispatch(Intent::RespondToInvitation {
        account: target.account.clone(),
        key: target.message_key.clone(),
        response: InvitationResponse::Accept,
        comment: None,
        notify_organizer: true,
        // The showcase's calendar schedules for itself, so no reply of ours is composed and no
        // subject is needed. Passing `None` is what a screenshot run does.
        reply_subject: None,
    });
    // The answer republishes the reading view, so wait for a Reading signal carrying the new one.
    let mut answered = None;
    while let Ok(surface) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(surface, Surface::Reading)
            && let Some(card) = app.reading_view().invitation
            && matches!(card.my_response, ResponseStatus::Accepted)
        {
            answered = Some(card);
            break;
        }
    }
    let card = answered.expect("the answer reached the card");
    assert_eq!(
        card.attendees.accepted, 3,
        "the tally moves with the answer, or the card contradicts itself"
    );
    assert_eq!(card.attendees.needs_action, 0);
}

#[test]
fn every_showcase_locale_translates_the_invitation() {
    // The invitation is assembled centrally (one message key, one roster, one meeting), so the
    // only thing a locale can leave behind is its *words*, and it would leave them behind
    // silently, as an English meeting title on a Dutch screenshot. The hold on the grid and the
    // iTIP payload read the same text, so comparing the holds covers both.
    let now = time::OffsetDateTime::now_utc();
    let titles: std::collections::BTreeSet<String> = ALL_SHOWCASE_LOCALES
        .iter()
        .map(|locale| {
            let text = crate::showcase_data::invite_text(*locale);
            format!(
                "{} · {} · {}",
                text.summary, text.location, text.description
            )
        })
        .collect();
    assert_eq!(
        titles.len(),
        ALL_SHOWCASE_LOCALES.len(),
        "two locales share the invitation's text; one was left untranslated"
    );

    for locale in ALL_SHOWCASE_LOCALES {
        let seed = crate::showcase_data::primary(locale, now);
        let key = showcase_invitation().message_key;
        assert!(
            seed.messages.iter().any(|m| m.id.key().as_str() == key),
            "{locale:?}: no seeded message keyed {key}"
        );
        // The mail's iTIP payload names the same meeting the calendar hold does. Getting this
        // wrong is not a missing card; it is a card whose conflict count and day preview
        // describe some other day.
        let ics = crate::showcase_data::invite_ics(locale, now);
        let hold = crate::showcase_data::primary_calendar(locale, now)
            .1
            .into_iter()
            .find(|event| ics.contains(&format!("UID:{}\r\n", event.uid.as_str())))
            .unwrap_or_else(|| panic!("{locale:?}: no calendar hold shares the invitation's UID"));
        assert_eq!(
            hold.title,
            crate::showcase_data::invite_text(locale).summary,
            "{locale:?}: the hold on the grid is titled differently from the card"
        );
        assert!(
            ics.contains(&format!("SUMMARY:{}\r\n", hold.title)),
            "{locale:?}: the iTIP summary differs from the hold's title"
        );
    }
}
