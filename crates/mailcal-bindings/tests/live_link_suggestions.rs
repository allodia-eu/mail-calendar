//! Gated live check of link suggestions against the harness, in the shape of the `stalwart-linked`
//! dev account: alice's mailbox used for mail alone, and alice's and bob's calendars as accounts of
//! their own. Stalwart lists alice's address in her principal's `calendar-user-address-set` and
//! bob's in his, so alice's calendar is the one suggested for her mailbox, from either end, and
//! bob's is offered without being suggested.
//!
//! Skips unless `STALWART_HTTP_ADDR` is set; `scripts/dev/harness.sh test` sets it. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 \
//!   cargo test -p mailcal-bindings --test live_link_suggestions -- --nocapture
//! ```

// Shared with the sign-in tests, which use the rest of it.
#[allow(dead_code)]
mod live_oauth;

use std::time::{Duration, Instant};

use live_oauth::{RecordingStore, app};
use mailcal_bindings::{AccountCapability, AccountEntry, CapabilityState, JmapSetup};

/// A calendar-and-contacts account for `username` on the harness.
fn dav(base: &str, username: &str, password: &str) -> String {
    format!(
        "[caldav]\nbase_url = \"{base}\"\nusername = \"{username}\"\npassword = \"{password}\"\n"
    )
}

fn uses_mail(entry: &AccountEntry) -> bool {
    entry
        .uses
        .iter()
        .any(|use_| use_.capability == AccountCapability::Mail && use_.state == CapabilityState::On)
}

#[test]
fn the_calendar_that_schedules_as_the_mailbox_address_is_suggested_and_a_strangers_is_not() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping link-suggestion live test: STALWART_HTTP_ADDR unset");
        return;
    };
    let base = format!("http://{http}");
    let app = app("link-suggestions", RecordingStore::default());

    let mailbox = mailcal_bindings::jmap_account_config_toml(JmapSetup {
        email: "alice@test.local".to_owned(),
        server_url: Some(base.clone()),
        password: "harness-alice-pw".to_owned(),
    })
    .expect("a JMAP config");
    let mailbox = app
        .add_account(format!("capabilities = [\"mail\"]\n{mailbox}"))
        .expect("alice's mailbox connects")
        .id;
    let alice = app
        .add_account(dav(&base, "alice@test.local", "harness-alice-pw"))
        .expect("alice's calendar connects")
        .id;
    let bob = app
        .add_account(dav(&base, "bob@test.local", "harness-bob-pw"))
        .expect("bob's calendar connects")
        .id;

    // The first snapshot asks each calendar server; a later one carries what they said.
    let deadline = Instant::now() + Duration::from_secs(30);
    let accounts = loop {
        let accounts = app.accounts_snapshot().accounts;
        let mail = accounts
            .iter()
            .find(|entry| uses_mail(entry))
            .expect("listed");
        if !mail.link_candidates.suggested.is_empty() || Instant::now() > deadline {
            break accounts;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let entry = |id: &str| {
        accounts
            .iter()
            .find(|entry| entry.id == id)
            .expect("listed")
    };

    let from_mailbox = &entry(&mailbox).link_candidates;
    let offered: Vec<&str> = from_mailbox
        .calendar
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect();
    assert!(offered.contains(&alice.as_str()) && offered.contains(&bob.as_str()));
    assert_eq!(
        from_mailbox.suggested,
        std::slice::from_ref(&alice),
        "only the calendar that schedules as alice"
    );

    assert_eq!(
        entry(&alice).link_candidates.suggested,
        std::slice::from_ref(&mailbox)
    );
    let from_bob = &entry(&bob).link_candidates;
    assert!(
        from_bob
            .mail
            .iter()
            .any(|candidate| candidate.id == mailbox)
    );
    assert!(
        from_bob.suggested.is_empty(),
        "bob's server does not schedule as alice"
    );
}
