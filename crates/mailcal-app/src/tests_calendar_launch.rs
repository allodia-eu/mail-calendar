//! What the calendar does at launch: paint the store, and fill it without being asked.
//!
//! The two halves are one behaviour and neither is worth much alone. Priming
//! ([`App::prime_calendar`]) can only show what the store holds, and until the launch fetch existed
//! the store was filled by nothing but the user opening the calendar tab: so a user who had never
//! opened it had no calendar at all, on any launch, online or off.
//!
//! The order matters as much as the pair: the stored week goes on screen first, and the round-trip
//! happens behind it. In the shipped app the two are deliberately in different places: the paint
//! is spawned at boot, the fetch waits in `reconnect_all` until every account has actually
//! connected: so these drive them in that order rather than through one entry point.

use std::sync::{Arc, Mutex, atomic::Ordering};

use fakes::{CalendarFake, calendar_account, calendar_app, calendar_app_on, event_from_today};
use mailcal_viewmodel::calendar::days::date_at;

use crate::Surface;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Today's date, in UTC: the anchor the fixtures are built around.
fn today() -> engine_api::CalendarDate {
    date_at(
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
                / 86_400,
        )
        .expect("in range"),
    )
}

/// A scratch store path that dies with the test.
fn scratch_store(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mailcal-launch-{name}-{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// **The point of the whole thing.** A launch fills the calendar store without being asked to, so
/// the *next* launch has something to paint; offline included.
///
/// Before this, the only thing that ever synced a calendar was the user opening the tab. Never open
/// it and the store stayed empty for good: no grid, no conflict count on an invitation, and nothing
/// at all once the network went away.
#[tokio::test]
async fn a_launch_fills_the_calendar_store_without_being_asked() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let db = scratch_store("unasked");

    // Launch one. Nobody opens the calendar; nobody dispatches anything.
    {
        let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
        let app = calendar_app_on(
            engine_api::Engine::open(&db).expect("open store"),
            vec![calendar_account("acct", provider)],
            &surfaces,
        );
        app.prime_calendar().await;
        app.refresh_calendar().await;
    }

    // Launch two, over the same store and with the network gone. Everything on screen now comes
    // from what launch one filed.
    let provider = CalendarFake::unreachable();
    let syncs = provider.syncs();
    let app = calendar_app_on(
        engine_api::Engine::open(&db).expect("reopen store"),
        vec![calendar_account("acct", provider)],
        &surfaces,
    );
    app.prime_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        page.is_materialized,
        "the stored week must be on screen at boot, offline or not"
    );
    assert_eq!(
        page.grid.timed.len(),
        1,
        "the stored occurrence was laid out"
    );
    assert_eq!(
        syncs.load(Ordering::Relaxed),
        0,
        "painting the grid went to the network"
    );
    let _ = std::fs::remove_file(&db);
}

/// The catch-up paints before it syncs, so the network is never in front of the grid.
///
/// Reversing the two would hand a returning user a blank grid for the length of a CalDAV round
/// trip over a store that held the week all along: the wait `prime_calendar` exists to remove.
#[tokio::test]
async fn the_catch_up_paints_the_stored_week_before_it_reaches_the_network() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let db = scratch_store("order");

    {
        let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
        let app = calendar_app_on(
            engine_api::Engine::open(&db).expect("open store"),
            vec![calendar_account("acct", provider)],
            &surfaces,
        );
        app.prime_calendar().await;
        app.refresh_calendar().await;
    }

    // A provider that cannot answer: every sync fails. The grid must still be up.
    let provider = CalendarFake::unreachable();
    let app = calendar_app_on(
        engine_api::Engine::open(&db).expect("reopen store"),
        vec![calendar_account("acct", provider)],
        &surfaces,
    );
    app.prime_calendar().await;
    app.refresh_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        page.is_materialized,
        "a failed sync must not take the stored week down with it"
    );
    assert_eq!(page.grid.timed.len(), 1);
    let _ = std::fs::remove_file(&db);
}

/// A launch with no network over a store that has never synced says "loading", not "nothing on".
///
/// This is the trap the catch-up walks into: the sync fails, the rebuild runs anyway, and the
/// window it claims is what `is_materialized` is derived from. Claiming it here would draw a
/// confidently empty week over a calendar nobody has ever read: the one lie `docs/calendar.md` §4
/// forbids, and it would be drawn on **every** launch, not just the first.
#[tokio::test]
async fn an_offline_launch_over_an_unsynced_store_says_loading_not_empty() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(
        vec![calendar_account("acct", CalendarFake::unreachable())],
        &surfaces,
    );

    app.prime_calendar().await;
    app.refresh_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        !page.is_materialized,
        "a calendar nobody could read must read as loading, never as an empty week"
    );
    assert!(page.grid.timed.is_empty());
}

/// The opposite lie, and it needs guarding just as much: an account with **no calendar provider**
/// is empty as a matter of fact.
///
/// Nothing will ever put a calendar in its store, so withholding the window would leave "loading
/// this period…" on screen for the life of the account.
#[tokio::test]
async fn a_mail_only_account_gets_an_answer_rather_than_a_permanent_loading() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut account = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    // Connected, and with nothing to connect a calendar to. The mail provider is what separates
    // this from a boot placeholder, which has no providers of any kind; see the test below.
    account.providers = vec![CalendarFake::with_events(Vec::new())];
    account.calendar_providers.clear();
    let app = calendar_app(vec![account], &surfaces);

    app.refresh_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        page.is_materialized,
        "an account that can never have a calendar has an empty one, not an unknown one"
    );
    assert!(page.grid.timed.is_empty());
}

/// A brand-new account gets its diary from the same first download that fetches its mail.
///
/// Its store is by definition empty, and the launch catch-up has already been and gone, so
/// without this the first session, the one where a meeting invitation is most likely to be read,
/// could only ever answer "we have not looked".
#[tokio::test]
async fn a_newly_added_accounts_first_sync_brings_its_calendar_with_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    // Nobody opens the calendar: this is the visible first download the host spawns on add.
    app.sync_added_account(&engine_api::AccountId::try_from("acct").unwrap())
        .await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(page.is_materialized, "the new account's week was read");
    assert_eq!(page.grid.timed.len(), 1, "its meeting is on the grid");
}

/// A refresh over accounts nobody has dialed yet claims nothing.
///
/// This is what an interactive launch looks like for its first second: the boot returns
/// **provider-less placeholders** so cached mail is on screen at once, and the dials land after.
/// A refresh in that window reaches nothing and files nothing, and if it claimed the window
/// anyway, the grid would draw a confidently empty week over a calendar nobody had looked at, and
/// the invitation card beside it would print a conflict count of zero it had no basis for.
///
/// It is also why the claim cannot simply be "no calendar provider means no calendar": a
/// placeholder and a mail-only account look identical by that test. What separates them is that a
/// placeholder has no providers **of any kind**.
#[tokio::test]
async fn a_refresh_before_the_accounts_are_dialed_claims_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut placeholder = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    placeholder.calendar_providers.clear();
    assert!(placeholder.providers.is_empty(), "the shape boot returns");
    let app = calendar_app(vec![placeholder], &surfaces);

    app.prime_calendar().await;
    app.refresh_calendar().await;

    let page = app.calendar_range(app.week_start_date(today()), 7);
    assert!(
        !page.is_materialized,
        "an account nobody has connected to yet has an unknown calendar, not an empty one"
    );
}

/// The catch-up tells the host once, when it has something to say.
///
/// The signal is a synchronous re-pull of every page on screen, so a launch that changed nothing
/// must not fire one; see `rebuild_calendar_cache`.
#[tokio::test]
async fn the_catch_up_signals_the_calendar_when_it_lands() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let provider = CalendarFake::with_events(vec![event_from_today("standup", 0, 9, 30)]);
    let app = calendar_app(vec![calendar_account("acct", provider)], &surfaces);

    app.prime_calendar().await;
    app.refresh_calendar().await;

    assert!(surfaces.lock().unwrap().contains(&Surface::Calendar));
    assert_eq!(
        app.calendar_range(app.week_start_date(today()), 7)
            .grid
            .timed
            .len(),
        1
    );
}
