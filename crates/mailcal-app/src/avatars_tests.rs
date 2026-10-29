//! Sender-photo resolution for the **mail** surfaces: the list and the reading header;
//! over a real in-memory engine.
//!
//! The harness is [`super::test_app`]; the contacts surface has its own suite beside this one.

use std::sync::{Arc, Mutex};

use super::test_app::{FakeContacts, PNG, app, card_with_photo, image_path, snapshot_from};
use crate::{ContactsIntent, Surface};

/// The whole path, and the property that regresses silently: **one** further snapshot.
///
/// A publish per resolved photo would signal every client once per face for a single
/// screenful, and nothing about the rendered result would look wrong, which is exactly why
/// the count is asserted rather than just the outcome.
#[tokio::test]
async fn a_photo_fills_in_and_publishes_exactly_one_snapshot() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    // A list on screen showing a monogram, then the cards arrive.
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    assert_eq!(
        image_path(&app.mailbox_list.get()),
        None,
        "nothing is known yet"
    );

    surfaces.lock().unwrap().clear();
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    assert_eq!(
        surfaces
            .lock()
            .unwrap()
            .iter()
            .filter(|surface| **surface == Surface::MailboxList)
            .count(),
        1,
        "one snapshot for the whole batch, not one per photo: {:?}",
        surfaces.lock().unwrap()
    );

    let path = image_path(&app.mailbox_list.get()).expect("the row now carries a photo");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        PNG,
        "and it is the fetched image"
    );

    // A resolved address is not asked about again.
    let mut snapshot = snapshot_from("ada@example.test");
    assert_eq!(app.attach_photos(&mut snapshot), []);
    assert!(image_path(&snapshot).is_some());
}

/// A sender nobody has a card for is the common case, and "nobody" is an answer. Recording it
/// is what stops every pass re-asking the provider about the same strangers.
#[tokio::test]
async fn a_sender_with_no_contact_is_asked_about_once() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    let mut snapshot = snapshot_from("stranger@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.resolve_sender_photos(wanted).await;

    let mut snapshot = snapshot_from("stranger@example.test");
    assert!(
        app.attach_photos(&mut snapshot).is_empty(),
        "a stranger is not re-queued on every rebuild"
    );
    assert_eq!(image_path(&snapshot), None);
}

/// A card that advertises a photo the source turns out not to hold resolves to a monogram,
/// not to an error and not to a retry loop.
#[tokio::test]
async fn a_card_whose_photo_is_absent_settles_on_the_monogram() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: None,
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    let mut snapshot = snapshot_from("ada@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.resolve_sender_photos(wanted).await;

    let mut snapshot = snapshot_from("ada@example.test");
    assert_eq!(app.attach_photos(&mut snapshot), []);
    assert_eq!(image_path(&snapshot), None);
}

/// Case is presentation, not identity: one lookup serves however a header spelled the address.
#[tokio::test]
async fn one_lookup_serves_every_spelling_of_the_same_address() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    let mut snapshot = snapshot_from("ada@EXAMPLE.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.resolve_sender_photos(wanted).await;

    let mut snapshot = snapshot_from("ada@example.test");
    assert!(
        app.attach_photos(&mut snapshot).is_empty(),
        "the domain is case-folded, so this is the same sender"
    );
}

/// **The ordering this got wrong in life.** The first snapshot is built before any card has
/// synced, so the first pass legitimately finds nobody, and recording that as "no contact"
/// is right, or every rebuild would re-ask about the same strangers. What is *not* right is
/// keeping it after contacts arrive half a second later: observed on macOS against the
/// harness, the pass ran at `…57.239` and the contact source bound at `…57.758`, and no
/// sender got a face for the rest of the session.
#[tokio::test]
async fn contacts_arriving_after_the_first_pass_still_get_their_photos() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );

    // A rebuild before any contact has synced: nobody is known, and that is recorded.
    let mut snapshot = snapshot_from("ada@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.resolve_sender_photos(wanted).await;
    let mut snapshot = snapshot_from("ada@example.test");
    assert!(
        app.attach_photos(&mut snapshot).is_empty(),
        "with no cards, one lookup settles it: a rebuild must not re-ask"
    );

    // The list is on screen, showing monograms.
    app.mailbox_list.publish(snapshot_from("ada@example.test"));

    // Contacts arrive. Nothing else will rebuild the mail list, so this is the only moment
    // that can put the face on the row: the whole path, not just the re-check.
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    assert!(
        image_path(&app.mailbox_list.get()).is_some(),
        "a card that synced after the first pass must still reach the row on screen"
    );
}

/// A contacts sync asks again about every photo, and keeps drawing the ones it knew until the
/// answer is in, so a photo that has not changed never leaves the row.
#[tokio::test]
async fn a_contacts_sync_keeps_drawing_a_known_face_while_it_asks_again() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    let known = image_path(&app.mailbox_list.get()).expect("the face resolved");

    app.recheck_sender_photos();
    let mut snapshot = snapshot_from("ada@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    assert_eq!(
        image_path(&snapshot).as_ref(),
        Some(&known),
        "a rebuild during the re-check still draws the face"
    );
    assert_eq!(wanted.len(), 1, "and the address is asked about again");

    app.resolve_sender_photos(wanted).await;
    let mut snapshot = snapshot_from("ada@example.test");
    assert!(
        app.attach_photos(&mut snapshot).is_empty(),
        "answered, so not asked again"
    );
    assert_eq!(image_path(&snapshot), Some(known));
}

/// A photo the re-check finds gone is taken off the rows on screen, not left drawn.
#[tokio::test]
async fn a_face_the_recheck_finds_gone_leaves_the_rows() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: None,
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    // What an earlier pass had found, before the source dropped the photo.
    app.avatar_photos.lock().unwrap().insert(
        "ada@example.test".to_owned(),
        super::PhotoState::Rechecking("/gone.png".to_owned()),
    );
    let mut snapshot = snapshot_from("ada@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.mailbox_list.publish(snapshot);
    assert!(image_path(&app.mailbox_list.get()).is_some());

    app.resolve_sender_photos(wanted).await;
    assert_eq!(
        image_path(&app.mailbox_list.get()),
        None,
        "the row on screen loses the photo the source no longer has"
    );
}

/// A pass publishes by what the list on screen draws, not by what the map held before it.
///
/// A rebuild attaches before a pass answers and can publish after it, so the list on screen can
/// lack a face the map already has. A pass that re-confirms the face must put it there; one that
/// finds the list already drawing it publishes nothing.
#[tokio::test]
async fn a_pass_puts_a_known_face_on_a_list_that_lacks_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    let known = image_path(&app.mailbox_list.get()).expect("the face resolved");
    let ada = || vec![engine_api::CanonicalEmail::parse("ada@example.test").unwrap()];

    surfaces.lock().unwrap().clear();
    app.resolve_sender_photos(ada()).await;
    assert!(
        !surfaces.lock().unwrap().contains(&Surface::MailboxList),
        "the same face already on screen is no news"
    );

    // A rebuild's list, attached before the face was known, published after it was.
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.resolve_sender_photos(ada()).await;
    assert_eq!(image_path(&app.mailbox_list.get()), Some(known));
}

/// Opening a message must not change the sender's face.
///
/// The reading header builds its own avatar, so built without the photo it *replaces* the
/// row's when the body snapshot lands: the list shows a photograph and opening the message
/// shows a monogram. Seen on macOS: the row drew the seeded photo and the header drew `BV`.
#[tokio::test]
async fn the_reading_header_draws_the_same_face_the_row_did() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;

    let row_photo = image_path(&app.mailbox_list.get()).expect("the row has a photo");
    // The header's own avatar, built the way opening a message builds it. Asserting the
    // *helper* would not catch this: the bug was the header not consulting it at all.
    let header = app.sender_avatar(Some(&engine_api::EmailAddress {
        name: Some("Ada Lovelace".into()),
        email: "ada@example.test".into(),
    }));
    assert_eq!(
        header.image_path.as_deref(),
        Some(row_photo.as_str()),
        "opening a message must not swap the photograph for a monogram"
    );
    assert_eq!(header.initials, "AL");
    assert_eq!(
        app.resolved_photo("ada@example.test").as_deref(),
        Some(row_photo.as_str())
    );
    // Case is not identity here either: a header spelling differs from a card's all the time.
    assert_eq!(
        app.resolved_photo("ada@EXAMPLE.test").as_deref(),
        Some(row_photo.as_str())
    );
    assert_eq!(app.resolved_photo("stranger@example.test"), None);
    assert_eq!(app.resolved_photo("not an address"), None);
}

/// A re-check the source cannot answer keeps the face: only an answer replaces it.
///
/// The contacts server is offline, so the adapter cannot be asked and nothing is cached for the
/// card. That is not the source saying the person has no photo.
#[tokio::test]
async fn a_recheck_the_source_cannot_answer_keeps_the_face() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: None,
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    // No connected account to ask, as when the app is offline.
    app.accounts.write().await.clear();
    app.avatar_photos.lock().unwrap().insert(
        "ada@example.test".to_owned(),
        super::PhotoState::Rechecking("/known.png".to_owned()),
    );
    let mut snapshot = snapshot_from("ada@example.test");
    let wanted = app.attach_photos(&mut snapshot);
    app.mailbox_list.publish(snapshot);

    app.resolve_sender_photos(wanted).await;
    assert_eq!(
        image_path(&app.mailbox_list.get()).as_deref(),
        Some("/known.png"),
        "a lookup that could not be asked leaves the face drawn"
    );
    assert_eq!(
        app.avatar_photos.lock().unwrap().get("ada@example.test"),
        Some(&super::PhotoState::File("/known.png".to_owned())),
        "and settled, so it is not asked again until the next contacts sync"
    );
}

/// A pass that read the people index before every answer was re-asked records nothing it found:
/// its answers come from the index the contacts sync replaced.
#[tokio::test]
async fn a_pass_from_before_a_recheck_records_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    let known = image_path(&app.mailbox_list.get()).expect("the face resolved");

    let before = app.avatar_pass.lock().unwrap().generation;
    app.recheck_sender_photos();
    let ada = vec![engine_api::CanonicalEmail::parse("ada@example.test").unwrap()];
    app.resolve_batch(ada, before).await;
    assert_eq!(
        app.avatar_photos.lock().unwrap().get("ada@example.test"),
        Some(&super::PhotoState::Rechecking(known)),
        "still to be asked about against the new index"
    );
}

/// A pass turned away because another was running is run by that one, so what it wanted is not
/// left until some later rebuild.
#[tokio::test]
async fn a_pass_turned_away_while_one_runs_is_run_by_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        FakeContacts {
            card: card_with_photo("ada@example.test"),
            photo: Some(PNG.to_vec()),
        },
        &surfaces,
    );
    app.dispatch(crate::Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    app.avatar_photos.lock().unwrap().clear();
    app.mailbox_list.publish(snapshot_from("ada@example.test"));
    let ada = vec![engine_api::CanonicalEmail::parse("ada@example.test").unwrap()];

    let running = app.begin_avatar_pass().expect("no pass yet");
    app.resolve_sender_photos(ada).await;
    assert_eq!(image_path(&app.mailbox_list.get()), None, "turned away");

    // The running pass finishing its own work.
    app.run_passes(Vec::new()).await;
    drop(running);
    assert!(
        image_path(&app.mailbox_list.get()).is_some(),
        "the turned-away pass's address is resolved by the pass that turned it away"
    );
}
