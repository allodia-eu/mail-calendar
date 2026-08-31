//! The same photo pass, seen from the **contacts** surfaces: the A–Z list and the detail one
//! of its rows opens.
//!
//! Split from `avatars_tests.rs` for the 500-line limit, and they belong apart: the failures
//! here are all of one shape: a face that reaches the mail list and stops short of a
//! contacts screen. The harness is [`super::test_app`].

use std::sync::{Arc, Mutex};

use super::test_app::{FakeContacts, PNG, app, card_with_photo, image_path, snapshot_from};

/// Most people in an address book never send the user mail, so a refresh that re-queued only
/// the mail list left every contacts-only row a monogram for the session.
///
/// Seen against the harness: after re-seeding, the two people who were *also* senders got their
/// faces and the four who only had cards did not.
#[tokio::test]
async fn a_contact_who_never_sent_mail_still_gets_their_photo() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            // Nothing in the mailbox is from this address; it exists only as a card.
            card: card_with_photo("never-writes@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    // A list of somebody else entirely, so the mail surface can never queue this person.
    app.mailbox_list
        .publish(snapshot_from("someone-else@example.test"));
    app.dispatch(crate::Intent::RefreshContacts).await;

    let contacts = app.contacts();
    let row = contacts
        .rows
        .iter()
        .find(|row| row.primary_email == "never-writes@example.test")
        .expect("the synced contact");
    assert!(
        row.avatar.image_path.is_some(),
        "a card with a photo must reach the contacts row even when its person sends no mail"
    );
}

/// A person is a person whether they are a sender or a row in the A–Z list, so the two
/// surfaces share one map and one fetch. Resolving them separately would mean two provider
/// round trips for one face, and, worse, the same person could end up with a photo on one
/// screen and a monogram on the other.
#[tokio::test]
async fn one_fetch_serves_both_the_mail_list_and_the_contacts_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.dispatch(crate::Intent::RefreshContacts).await;

    // The mail row has its face…
    assert!(image_path(&app.mailbox_list.get()).is_some());
    // …and so does the contacts row, from the same resolved answer.
    let contacts = app.contacts();
    let row = contacts
        .rows
        .iter()
        .find(|row| row.primary_email == "ada@example.test")
        .expect("the synced contact");
    assert!(
        row.avatar.image_path.is_some(),
        "the contacts row draws the same photo the mail row does"
    );
    assert_eq!(row.avatar.initials, "AL");
}

/// Opening a contact must not change their face: the same defect the reading header had, on
/// the screen where a photograph is most of the content.
///
/// The detail is projected from the engine's person, which knows nothing of the resolved-photo
/// map, so it arrives with the monogram and the row's photo never reaches it. Seen on macOS:
/// the harness contacts list drew six faces and every one of them opened onto initials.
#[tokio::test]
async fn opening_a_contact_draws_the_same_face_the_row_did() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshContacts).await;

    let row = app
        .contacts()
        .rows
        .into_iter()
        .find(|row| row.primary_email == "ada@example.test")
        .expect("the synced contact");
    let row_photo = row.avatar.image_path.expect("the row has a photo");

    let detail = app.contact_detail(&row.id).await.expect("it opens");
    assert_eq!(
        detail.avatar.image_path.as_deref(),
        Some(row_photo.as_str()),
        "tapping a contact must not swap the photograph for a monogram"
    );
    assert_eq!(detail.avatar.initials, "AL");
}
