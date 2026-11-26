//! What a refused calendar write says about itself: the failure's class, never the server's
//! words. A CalDAV error body can name the resource the write addressed, and that address carries
//! the meeting's `UID`; the reason an answer failed is logged (`docs/logging.md`).

use std::sync::{Arc, Mutex};

use fakes::{InvitationFake, invitation_app};

use super::{Intent, InvitationResponse, invite};
use crate::tests_fakes as fakes;

/// Answers the fixture invitation over `provider` and returns why it failed.
async fn refusal(provider: InvitationFake) -> String {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = invitation_app(provider, &surfaces);
    Box::pin(app.dispatch(Intent::RefreshMail)).await;
    Box::pin(app.dispatch(Intent::RefreshCalendar)).await;
    Box::pin(app.respond_to_invitation(&invite(), InvitationResponse::Accept, None, true, None))
        .await
        .expect_err("the calendar refused the write")
}

#[tokio::test]
async fn a_refused_store_names_no_address_in_its_reason() {
    let reason = refusal(
        InvitationFake::new()
            .without_server_scheduling()
            .refusing_writes(),
    )
    .await;
    assert_eq!(
        reason,
        "the meeting could not be added to your calendar: the server refused it"
    );
}

#[tokio::test]
async fn a_refused_answer_names_no_address_in_its_reason() {
    // The server route: the meeting is on the calendar and the answer is written to it.
    let reason = refusal(InvitationFake::new().refusing_writes()).await;
    assert_eq!(
        reason,
        "the answer could not be stored on the calendar: the server refused it"
    );
}
