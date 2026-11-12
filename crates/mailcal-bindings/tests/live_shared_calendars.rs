//! Gated live check of shared stores holding calendars against the harness, where alice may write
//! the `support` group's calendar and read the one dave shares with her
//! (`docker/stalwart/seed.sh`), over JMAP and over CalDAV: the server lists both with what each
//! holds, choosing one lists it as an account of its own with that store's calendars and rights, in
//! the mail surfaces only when it holds mail, the choice is stored with alice's account, and
//! switching it, or alice's calendar, off takes its calendars out again
//! (`docs/shared-mailboxes.md`).
//!
//! Skips unless `STALWART_HTTP_ADDR` is set. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 \
//!   cargo test -p mailcal-bindings --test live_shared_calendars -- --nocapture --test-threads=1
//! ```

// Shared with the sign-in tests, which use the rest of it.
#[allow(dead_code)]
mod live_oauth;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use live_oauth::{RecordingStore, app};
use mailcal_bindings::{
    CalendarRow, JmapSetup, MailcalApp, SharedStoreEntry, SharedStoreSupport, SharedUse, SharedUses,
};

const SUPPORT: &str = "support@test.local";
const DAVE: &str = "dave@test.local";
/// The calendar dave shares with alice, read-only.
const DAVE_SHARED: &str = "Dave Shared";

/// Waits up to a minute for `check` to answer something, polling.
fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn calendars_of(app: &MailcalApp, address: &str) -> Vec<CalendarRow> {
    app.calendars()
        .into_iter()
        .filter(|row| row.account_address == address)
        .collect()
}

/// The stores `parent` is offered, once its calendar has connected.
fn offered(app: &MailcalApp, parent: &str) -> Vec<SharedStoreEntry> {
    let view = eventually("the account's calendar to list its shared stores", || {
        let view = app
            .shared_stores(parent.to_owned())
            .inspect_err(|err| eprintln!("not yet: {err}"))
            .ok()?;
        (view.support != SharedStoreSupport::NotConnected).then_some(view)
    });
    assert_eq!(view.support, SharedStoreSupport::Listed, "{view:?}");
    view.stores
}

fn entry(stores: &[SharedStoreEntry], address: &str) -> SharedStoreEntry {
    stores
        .iter()
        .find(|store| store.address.as_deref() == Some(address))
        .cloned()
        .unwrap_or_else(|| panic!("{address} is offered: {stores:?}"))
}

/// Whether `account` is listed in the mail surfaces.
fn in_mail(app: &MailcalApp, account: &str) -> bool {
    app.mailbox_list()
        .accounts
        .iter()
        .any(|row| row.id == account)
}

/// Chooses dave's store and the group's for `parent`, checks what that does, and switches them
/// off again: dave's alone, then the group's calendars with alice's own calendar when she has mail
/// to keep the account (`with_mail`), the group's store on its own otherwise.
fn the_shared_calendars_are_listed_chosen_synced_stored_and_dropped(
    app: &Arc<MailcalApp>,
    store: &RecordingStore,
    parent: &str,
    with_mail: bool,
) {
    let calendar = SharedUses::of(SharedUse::Calendar);
    let stores = offered(app, parent);
    let (dave, support) = (entry(&stores, DAVE), entry(&stores, SUPPORT));
    assert!(!dave.selected && dave.listed);
    // Where the account opens mail, a store holding mail and calendars is one store with both:
    // the group's mailbox and calendar, and dave's Board folder and his calendar. A CalDAV
    // account opens their calendars alone.
    let holds = if with_mail {
        calendar.with(SharedUse::Mail)
    } else {
        calendar
    };
    assert_eq!(dave.holds, holds, "{dave:?}");
    assert_eq!(support.holds, holds, "{support:?}");
    // Bob shares mail alone.
    assert!(
        stores
            .iter()
            .filter(|store| store.address.as_deref() == Some("bob@test.local"))
            .all(|bob| !bob.holds.contains(SharedUse::Calendar)),
        "{stores:?}"
    );

    app.set_shared_store(parent.to_owned(), dave.clone(), true)
        .expect("choosing dave's store");
    app.set_shared_store(parent.to_owned(), support.clone(), true)
        .expect("choosing the group's store");
    let dave_calendar = eventually("dave's calendar to sync", || {
        calendars_of(app, DAVE)
            .into_iter()
            .find(|row| row.name == DAVE_SHARED)
    });
    assert!(!dave_calendar.can_write, "dave shares it read-only");
    assert_ne!(dave_calendar.account, parent, "an account of its own");
    // Each holds its owner's calendars and none of alice's: a store opened as the credential's
    // own would list her Work calendar under dave's name.
    let names = |address| -> Vec<String> {
        calendars_of(app, address)
            .into_iter()
            .map(|row| row.name)
            .collect()
    };
    assert_eq!(names(DAVE), [DAVE_SHARED]);
    let group = eventually("the group's calendar to sync", || {
        calendars_of(app, SUPPORT).into_iter().next()
    });
    assert!(group.can_write, "alice may write the group's calendar");
    assert!(
        !names(SUPPORT).iter().any(|name| name == "Work"),
        "{:?}",
        names(SUPPORT)
    );
    // A store is in the mail surfaces only when it opens mail: over JMAP as one account with its
    // calendar, over CalDAV not at all.
    assert_eq!(in_mail(app, &dave_calendar.account), with_mail);
    assert_eq!(in_mail(app, &group.account), with_mail);
    let stored = store.0.lock().unwrap().last().cloned().unwrap();
    assert_eq!(stored.matches("[[shared_store]]").count(), 2, "{stored}");

    app.set_shared_store(parent.to_owned(), dave, false)
        .expect("switching dave's store off");
    assert!(calendars_of(app, DAVE).is_empty(), "switched off, it goes");
    assert!(!calendars_of(app, SUPPORT).is_empty(), "the other stays");

    if with_mail {
        // Alice's own calendar opens her shared ones, so they go with it; the group's mailbox
        // stays, and goes when it is switched off.
        app.set_account_capability(
            parent.to_owned(),
            mailcal_bindings::AccountCapability::Calendar,
            false,
        )
        .expect("switching alice's calendar off");
        assert!(calendars_of(app, SUPPORT).is_empty());
        assert!(in_mail(app, &group.account), "its mail stays");
    }
    app.set_shared_store(parent.to_owned(), support, false)
        .expect("switching the group's store off");
    assert!(calendars_of(app, SUPPORT).is_empty());
    assert!(!in_mail(app, &group.account));
    let stored = store.0.lock().unwrap().last().cloned().unwrap();
    assert!(!stored.contains("[[shared_store]]"), "{stored}");
}

#[test]
fn a_jmap_account_opens_the_calendars_shared_with_it() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping shared-calendar live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("shared-calendars-jmap", store.clone());
    let config = mailcal_bindings::jmap_account_config_toml(JmapSetup {
        email: "alice@test.local".to_owned(),
        server_url: Some(format!("http://{http}")),
        password: "harness-alice-pw".to_owned(),
    })
    .expect("a JMAP config");
    let parent = app.add_account(config).expect("alice connects").id;
    the_shared_calendars_are_listed_chosen_synced_stored_and_dropped(&app, &store, &parent, true);
}

#[test]
fn a_caldav_account_opens_the_calendars_shared_with_it() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping shared-calendar live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("shared-calendars-caldav", store.clone());
    let config = format!(
        "[caldav]\nbase_url = \"http://{http}\"\nusername = \"alice@test.local\"\n\
         password = \"harness-alice-pw\"\n"
    );
    let parent = app
        .add_account(config)
        .expect("alice's calendar connects")
        .id;
    the_shared_calendars_are_listed_chosen_synced_stored_and_dropped(&app, &store, &parent, false);
}
