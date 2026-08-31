//! Test fixtures for the runtime/loop tests: the JMAP-shaped [`FakeProvider`], the recording
//! observer, the typed-reference helpers, and the `account`/`app`/`message`/`flat_subjects`
//! builders. The calendar provider and on-demand connectors live in the `calendar` and
//! `connectors` submodules (re-exported here) so each file stays under the 500-line limit;
//! the items the tests use are `pub(super)` so the parent test module can reach them.
//!
//! Included by more than one test file (`tests.rs`, `connectivity_tests.rs`), so not every
//! consumer uses every fixture; `dead_code` and unused submodule re-exports are allowed
//! rather than gated per item and per consumer.
#![allow(dead_code, unused_imports)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    ids::{MailboxId, MessageId, MessageIdHeader, ThreadId},
    mail::{Mailbox, MailboxRole, Message},
    membership::Memberships,
    raw::RawMime,
    sync::{JmapDataType, SyncScope, SyncState, SyncUpdate, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, EmailChunk, EmailStream, MailEdit, MailEditReceipt, Provider,
    ProviderError, ProviderResult, ScopeSync,
};
use mailcal_viewmodel::{MailboxListSnapshot, SnapshotRow};
use tokio::sync::Notify;

use crate::{
    Account, App, AppObserver, EventRef, FolderRef, Intent, MailboxConnector, MessageRef, Surface,
    Telemetry, ThreadRef, TimeZoneInit,
};

// This whole file is `#[path]`-included by several test modules, so these submodules load more
// than once: the intended shared-test-helper shape, not a real duplicate.
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/calendar.rs"]
mod calendar;
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/calendar_builders.rs"]
mod calendar_builders;
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/connectors.rs"]
mod connectors;
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/fixtures.rs"]
mod fixtures;
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/invitation.rs"]
mod invitation;
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes/provider.rs"]
mod provider;

pub(super) use calendar::CalendarFake;
pub(super) use calendar_builders::{
    calendar_account, calendar_app, calendar_app_on, event_from_today, occurrence_wall_clock_of,
    stored_event, weekly_event_from_today, weekly_event_with_a_moved_occurrence,
};
pub(super) use connectors::{FakeConnector, FlakyConnector, ObservingConnector};
pub(super) use fixtures::{evt, message, msg, open_folder, thread_ref, threaded, unthreaded};
pub(super) use invitation::{
    ALIAS, EVENT_KEY, InvitationFake, MEETING_UID, MESSAGE_KEY, RecordedPuts,
    RecordedRsvps as InvitationRsvps, RecordedSends, invitation_app, invitation_app_with_prefs,
};
pub(super) use provider::FakeProvider;

/// Records the surfaces the app signals, so the test can assert the loop fired.
struct RecordingObserver {
    surfaces: Arc<Mutex<Vec<Surface>>>,
}

impl AppObserver for RecordingObserver {
    fn surface_changed(&self, surface: Surface) {
        self.surfaces.lock().unwrap().push(surface);
    }
}

/// Wraps `provider` as an account `id` (its identity is `me@<id>.local`, mail only).
pub(super) fn account(id: &str, provider: FakeProvider) -> Account<FakeProvider> {
    account_with(id, vec![provider])
}

/// An account `id` whose mail is served by several folder-scoped providers, as a real account's is
/// : so a test can give one scope a different fate from its siblings.
pub(super) fn account_with(id: &str, providers: Vec<FakeProvider>) -> Account<FakeProvider> {
    Account {
        id: AccountId::try_from(id).unwrap(),
        providers,
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new(format!("me@{id}.local")),
    }
}

/// Builds an in-memory app over `accounts`, recording signalled surfaces into `surfaces`.
pub(super) fn app(
    accounts: Vec<Account<FakeProvider>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<FakeProvider> {
    App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    )
}

/// Builds an in-memory app over `accounts` whose preferences persist to `prefs_path`, so a
/// test can prove a setting survives a relaunch (a fresh app over the same path). The device
/// zone is UTC (no timezone prompt), so only the setting under test drives behaviour.
pub(super) fn app_with_prefs(
    accounts: Vec<Account<FakeProvider>>,
    prefs_path: std::path::PathBuf,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<FakeProvider> {
    App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: Some(prefs_path.clone()),
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        // The analytics consent decision lives in the same preferences file, so hand telemetry
        // the same path: a test can then prove a consent choice survives a relaunch. Still
        // `off` (no device, no sink) so nothing is ever built or sent from a test.
        Telemetry::off(Some(prefs_path)),
    )
}

/// Builds an in-memory app over `accounts` with an on-demand `connector`, recording
/// signalled surfaces into `surfaces`.
pub(super) fn app_with_connector<C: MailboxConnector<FakeProvider> + 'static>(
    accounts: Vec<Account<FakeProvider>>,
    connector: C,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<FakeProvider> {
    App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        Some(Box::new(connector)),
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    )
}

pub(super) fn flat_subjects(snapshot: &MailboxListSnapshot) -> Vec<String> {
    snapshot
        .rows
        .iter()
        .filter_map(|row| match row {
            SnapshotRow::Flat(r) => Some(r.subject.clone()),
            SnapshotRow::Thread(_) => None,
        })
        .collect()
}

pub(super) fn flat_previews(snapshot: &MailboxListSnapshot) -> Vec<String> {
    snapshot
        .rows
        .iter()
        .filter_map(|row| match row {
            SnapshotRow::Flat(r) => Some(r.preview.clone()),
            SnapshotRow::Thread(_) => None,
        })
        .collect()
}
