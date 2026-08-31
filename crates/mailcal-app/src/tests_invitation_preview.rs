//! "Around this meeting" over a calendar that does not hold the meeting, end to end.
//!
//! The picture answers *"where would this land in my day"*, and on a mailbox nothing files an
//! invitation into: a bare IMAP account, or an IMAP+CalDAV one whose server has no bridge from
//! the mail store: the one block the card is about was the one block missing. Which made the
//! same invitation draw a different picture depending on the server behind it.
//!
//! The rule itself is a pure function with its own table (`invitations_conflict_tests.rs`);
//! these prove the block actually reaches a client, at the meeting's own times, and that a
//! calendar which *does* hold the meeting still draws it exactly once.

use std::sync::{Arc, Mutex};

use fakes::{EVENT_KEY, InvitationFake, MESSAGE_KEY, invitation_app};
use mailcal_viewmodel::ResponseStatus;

use super::{Intent, MessageRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

fn invite() -> MessageRef {
    MessageRef {
        account: engine_api::AccountId::try_from("acct-a").unwrap(),
        key: engine_api::ProviderKey::new(MESSAGE_KEY).unwrap(),
    }
}

/// Boots an app over `provider`, syncs both sides, opens the invitation and returns its card.
async fn card(provider: InvitationFake) -> mailcal_viewmodel::InvitationCard {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::RefreshCalendar).await;
    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;
    app.reading_view()
        .invitation
        .expect("the message carries an invitation")
}

#[tokio::test]
async fn a_meeting_no_calendar_holds_is_still_drawn_around_itself() {
    // **The reported bug.** The preview was built from the diary read alone, so a meeting nothing
    // had filed simply was not in it: the card said "Nothing else in your calendar then" over a
    // drawn, entirely empty day, and the reader had to place the meeting in it themselves.
    let card = card(InvitationFake::new().without_the_meeting()).await;

    assert!(
        card.conflicts_known,
        "the calendar was read, which is what lets the preview be drawn at all"
    );
    assert_eq!(
        card.conflict_count, 0,
        "the meeting is not a clash with itself: the count is the picture's, minus this block"
    );
    assert_eq!(
        card.preview.timed.len(),
        1,
        "the meeting the card is about is on its own day preview"
    );
    let block = &card.preview.timed[0];
    assert_eq!(block.title, "Sprint planning");
    assert_eq!(
        block.participation,
        ResponseStatus::NeedsAction,
        "unanswered, so every client draws it dotted: the hold treatment it already has"
    );
    // The app's display zone is UTC, and the meeting is 09:00–10:00Z: the block sits at the times
    // the card states two rows above it, so the picture cannot contradict its own "When".
    assert_eq!(block.start_minutes, 9 * 60);
    assert_eq!(block.end_minutes, 10 * 60);
    assert!(
        block.event.is_empty() && block.calendar.is_empty(),
        "there is no stored event to key and no calendar to colour it by"
    );
    assert!(!block.can_write, "nothing was written, so nothing is ours");
}

#[tokio::test]
async fn a_meeting_the_calendar_holds_is_drawn_once_and_from_the_calendar() {
    // The other half, and the one a bug here would break silently: on every server that files an
    // invitation, the stored copy was already drawn. A second, synthesized block would double
    // every invitation on every account that works today.
    let card = card(InvitationFake::new()).await;

    assert_eq!(card.preview.timed.len(), 1, "one meeting, one block");
    assert_eq!(
        card.preview.timed[0].event, EVENT_KEY,
        "and it is the calendar's copy, which is the one that can change"
    );
}

#[tokio::test]
async fn an_unread_calendar_draws_nothing_at_all() {
    // A block over a day nobody has read is indistinguishable from a day with only that block on
    // it. `docs/calendar.md` §4: the count says "we have not looked", and the client withholds the
    // grid: so the meeting must not be smuggled into the picture through this path either.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(InvitationFake::new().without_the_meeting(), &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    // No RefreshCalendar: mail syncs first, so this is what a cold start actually looks like.
    app.dispatch(Intent::OpenMessage { message: invite() })
        .await;

    let card = app
        .reading_view()
        .invitation
        .expect("the message carries an invitation");
    assert!(!card.conflicts_known, "the calendar has not been read");
    assert!(
        card.preview.timed.is_empty(),
        "so there is no picture to put the meeting in"
    );
}
