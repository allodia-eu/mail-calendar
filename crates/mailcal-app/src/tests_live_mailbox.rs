//! Live-commit re-projection tests (`live_mailbox.rs`): the streamed-sync path that republishes
//! the visible mailbox list from the in-memory cached rows, without re-reading the store.
//!
//! The rule under test is that this fast path may only ever publish a list it can build
//! **completely**. It projects the *whole* visible list from that cache, so a dropped cache would
//! leave the list missing rows until the authoritative rebuild at the end of the sync pass put
//! them back a second or two later. On screen that is the whole list flashing to a shorter one and
//! back, which is exactly what it looked like when it was reported on Windows.
//!
//! The other half of that report (the list *jumping*) is here too: the download bar sits above
//! the list on every client, so raising it for the sync that follows the user's own archive
//! shoves the whole list down and back up. That pass now stays quiet.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use fakes::{FakeProvider, account, app, flat_previews, flat_subjects, message, msg};
use mailcal_viewmodel::ViewMode;

use super::{Intent, Surface, sync::STREAM_CHUNK_SIZE};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A reconcile pass drops the live cache (its tombstones name no keys, so the cache cannot be
/// spliced). The **next** streamed commit must not republish a list built from that hole: every
/// row would disappear until the authoritative rebuild lands.
#[tokio::test]
async fn a_live_commit_keeps_the_rows_of_a_list_whose_cache_was_dropped() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "acct-1",
                FakeProvider::with(vec![message("m1", "a", "From one")]),
            ),
            account(
                "acct-2",
                FakeProvider::with(vec![message("m2", "a", "From two")]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    let before = flat_subjects(&app.mailbox_list());
    assert!(before.contains(&"From one".to_owned()) && before.contains(&"From two".to_owned()));

    app.invalidate_list_cache();

    let one = AccountId::try_from("acct-1").unwrap();
    app.apply_live_mail_delta(&one, &[message("m1", "a", "From one")], &[], 0);

    assert!(
        flat_subjects(&app.mailbox_list()).contains(&"From two".to_owned()),
        "acct-2's rows must survive a live commit for another account; they were: {:?}",
        flat_subjects(&app.mailbox_list()),
    );
}

/// The live path reads no folders; it rebuilds rows from the cached list and carries the
/// sidebar's data over from the snapshot on screen. The All Inboxes badge is part of that data,
/// derived from folders this path was handed none of, so recomputing it here yields zero.
///
/// On screen that is the badge blinking out on every optimistic update (an archive, a swipe, a
/// mark-read) and coming back a beat later when the authoritative rebuild lands, which reads as
/// a flicker nobody can reproduce on demand, because it needs a live commit to be in flight.
#[tokio::test]
async fn a_live_commit_keeps_the_all_inboxes_badge_it_did_not_recompute() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with_unread(vec![message("m1", "a", "From one")], 5),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        app.mailbox_list().unified_unread,
        5,
        "the authoritative rebuild carries the server's count"
    );

    let one = AccountId::try_from("acct-1").unwrap();
    app.apply_live_mail_delta(&one, &[message("m1", "a", "From one")], &[], 0);

    assert_eq!(
        app.mailbox_list().unified_unread,
        5,
        "a live commit must not blank the badge it never recomputed"
    );
}

/// The same hole from the other side: the commit is for the only account showing. Its delta is
/// not its mailbox, so seeding the dropped cache from it and republishing would shrink the list to
/// the one message that just landed.
#[tokio::test]
async fn a_live_commit_over_a_dropped_cache_does_not_shrink_the_list_to_its_delta() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![
                message("m1", "a", "First"),
                message("m2", "a", "Second"),
                message("m3", "a", "Third"),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(flat_subjects(&app.mailbox_list()).len(), 3);

    let one = AccountId::try_from("acct-1").unwrap();
    app.invalidate_list_cache();
    app.apply_live_mail_delta(&one, &[message("m2", "a", "Second")], &[], 0);

    assert_eq!(
        flat_subjects(&app.mailbox_list()).len(),
        3,
        "the list must still show the mailbox, not the delta; it was: {:?}",
        flat_subjects(&app.mailbox_list()),
    );
}

/// The guard must not switch the live path off for the case it exists to serve: a first sync
/// over an empty cache still paints rows as they stream in (nothing is on screen to lose).
#[tokio::test]
async fn a_first_commit_still_paints_before_the_authoritative_rebuild() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m1", "a", "First")]),
        )],
        &surfaces,
    );
    // Discover the inbox key (the unified projection needs it) without any mail landing yet.
    app.dispatch(Intent::RefreshMail).await;
    app.invalidate_list_cache();
    // Reloading clears the staleness, as the pass's own rebuild does.
    app.dispatch(Intent::SetViewMode(ViewMode::Flat)).await;

    app.apply_live_mail_delta(
        &AccountId::try_from("acct-1").unwrap(),
        &[message("m9", "a", "Streamed in")],
        &[],
        0,
    );
    assert!(
        flat_subjects(&app.mailbox_list()).contains(&"Streamed in".to_owned()),
        "a commit over a live cache must still splice its rows in",
    );
}

/// Archiving re-commits the message it moved, so the follow-up sync does download, which is why
/// it has to be told to stay silent on **both** surfaces. The twin of
/// `reset_shows_the_download_bar_even_when_mail_is_already_listed`, which pins the case that
/// *must* show the bar.
#[tokio::test]
async fn archiving_announces_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // `with_archive`, not `with`: without an Archive folder to resolve, the intent is a no-op
    // that never reaches the follow-up sync at all, and the assertion below could not fail.
    let provider = FakeProvider::with_archive(vec![
        message("m1", "a", "First"),
        message("m2", "a", "Second"),
    ]);
    let late = provider.late_delivery();
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    app.dispatch(Intent::RefreshMail).await;
    assert!(!app.sync_progress().active);

    // Give the archive's follow-up sync something to download. That is not a contrivance: it is
    // what makes this a check that can fail: the real pass downloads too (the archived message
    // is re-committed in its new folder), and a downloading pass is exactly what a background
    // pass would announce.
    late.lock()
        .unwrap()
        .push(message("m3", "a", "Landed later"));

    // Poll both surfaces for as long as the archive's own sync is in flight, like the reset test.
    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.dispatch(Intent::Archive {
                message: msg("acct-1", "m1"),
            })
            .await;
        }
    });
    let mut saw_bar = false;
    let mut saw_hint = false;
    while !task.is_finished() {
        let progress = app.sync_progress();
        saw_bar |= progress.active;
        saw_hint |= !progress.accounts.is_empty();
        if saw_bar && saw_hint {
            break;
        }
        tokio::task::yield_now().await;
    }
    task.await.unwrap();

    assert!(
        !saw_bar,
        "the sync that follows the user's own archive must not raise the download bar",
    );
    assert!(
        !saw_hint,
        "and must not name the account in the background hint either",
    );
}

/// The pane's expansion, end to end through the intent the clients dispatch: it flips the account
/// row, leaves the neighbouring account alone, and moves neither the selected account nor folder.
///
/// The last part is the rule a client cannot check for itself; `SetAccountExpanded` sharing any
/// code with `SelectAccount` would make every chevron a navigation, which is the behaviour this
/// whole change exists to undo.
#[tokio::test]
async fn expanding_an_account_flips_only_that_account_and_navigates_nowhere() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "acct-1",
                FakeProvider::with(vec![message("m1", "a", "One")]),
            ),
            account(
                "acct-2",
                FakeProvider::with(vec![message("m2", "a", "Two")]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;

    // Every account starts expanded; nobody has shut one.
    assert!(app.mailbox_list().accounts.iter().all(|a| a.expanded));

    app.dispatch(Intent::SetAccountExpanded {
        account: "acct-2".to_owned(),
        expanded: false,
    })
    .await;

    let snapshot = app.mailbox_list();
    let expanded: Vec<(String, bool)> = snapshot
        .accounts
        .iter()
        .map(|a| (a.id.clone(), a.expanded))
        .collect();
    assert_eq!(
        expanded,
        vec![("acct-1".to_owned(), true), ("acct-2".to_owned(), false)],
        "only the named account shuts"
    );
    // Expanding is not navigating.
    assert_eq!(snapshot.selected_account.as_deref(), Some("acct-1"));
    assert!(snapshot.selected.is_none());

    // …and it comes back.
    app.dispatch(Intent::SetAccountExpanded {
        account: "acct-2".to_owned(),
        expanded: true,
    })
    .await;
    assert!(app.mailbox_list().accounts.iter().all(|a| a.expanded));
}

/// A message re-sent whole must not lose the snippet the body sync computed for it.
///
/// A provider with no server preview (IMAP) carries `None` on every object it sends, which means
/// "nothing to say", not "clear it": the store keeps what it holds, so the spliced row has to as
/// well or the list disagrees with the store it stands in for. On screen the row's second line
/// blanks for the length of the pass and then comes back.
#[tokio::test]
async fn a_live_commit_keeps_a_preview_the_resent_message_does_not_carry() {
    let mut seeded = message("m1", "a", "Quarterly report");
    seeded.preview = Some("The numbers you asked for are attached.".to_owned());
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", FakeProvider::with(vec![seeded]))],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        flat_previews(&app.mailbox_list()),
        vec!["The numbers you asked for are attached.".to_owned()],
        "the seeded snippet is on the row to begin with"
    );

    // The same message again, as any resync re-sends it: same content, and silence on the
    // snippet the provider never had.
    let resent = message("m1", "a", "Quarterly report");
    assert!(resent.preview.is_none());
    let account_id = AccountId::try_from("acct-1").unwrap();
    app.apply_live_mail_delta(&account_id, &[resent], &[], 0);

    assert_eq!(
        flat_previews(&app.mailbox_list()),
        vec!["The numbers you asked for are attached.".to_owned()],
        "the snippet survives the re-send"
    );
}

/// A streamed pass re-projects the visible list once per **commit chunk**, not once per message.
///
/// Every commit clones the cached window, scans it per upserted message, re-sorts it and rebuilds
/// the visible snapshot from it. At one message per chunk a first sync therefore pays that for
/// every message it downloads; on a measured five-account sync of 7,107 messages it held a core
/// at 100% for three minutes, and parked the reading pane behind the backlog for 2 min 24 s.
///
/// The debounce in `mailcal-bindings` cannot cover this: it caps what reaches the *host*, and this
/// work happens on the sync thread before any signal is emitted.
#[tokio::test]
async fn a_streamed_pass_reprojects_the_list_once_per_chunk_not_once_per_message() {
    // Enough mail that one rebuild per message would be unmistakable, and deliberately **not** a
    // multiple of the chunk size: the last chunk of a pass is a remainder, and a chunking that
    // only emitted full chunks would silently drop it: the whole tail of every mailbox.
    const DOWNLOADED: usize = 203;

    assert!(
        !DOWNLOADED.is_multiple_of(STREAM_CHUNK_SIZE),
        "the fixture must leave a remainder, or the last-chunk case goes untested",
    );
    let mail: Vec<_> = (0..DOWNLOADED)
        .map(|n| message(&format!("m{n}"), "a", "Catching up"))
        .collect();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::with(mail))], &surfaces);

    app.dispatch(Intent::RefreshMail).await;

    let published = surfaces
        .lock()
        .unwrap()
        .iter()
        .filter(|surface| **surface == Surface::MailboxList)
        .count();
    // Deliberately **not** derived from `STREAM_CHUNK_SIZE`: a ceiling that scales with the
    // chunk would be satisfied by any chunk at all, including the one-message chunk this exists
    // to rule out. The rule is that a pass rebuilds the list a handful of times, whatever the
    // chunk is tuned to.
    let ceiling = DOWNLOADED / 10;
    assert!(
        published <= ceiling,
        "{DOWNLOADED} messages published {published} lists; a bulk commit \
         ({STREAM_CHUNK_SIZE} per chunk) owes at most {ceiling}"
    );
    assert_eq!(
        app.mailbox_list().total,
        DOWNLOADED,
        "every downloaded message still reaches the list"
    );
}
