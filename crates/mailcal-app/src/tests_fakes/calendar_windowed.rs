//! A [`CalendarFake`] that fetches inside a window, as Microsoft's and Google's calendars do: its
//! snapshot lists only the events inside the window the engine names, so a page past the first
//! sync's window holds nothing the server was not asked for again.

use std::sync::{Arc, Mutex};

use engine_api::Horizon;
use engine_core::{
    calendar::Event,
    sync::SyncUpdate,
    time::{CalendarDateTime, UtcDateTime},
};
use engine_provider::ScopeSync;
use tokio::sync::Notify;

use super::calendar::CalendarFake;

/// How a windowed [`CalendarFake`] fetches: every window it was asked for, and an optional gate
/// that parks a fetch (holding its calendar) until the test lets it go.
#[derive(Default)]
pub(crate) struct WindowedFetch {
    /// Every window an events fetch named, in order.
    pub(crate) windows: Mutex<Vec<Horizon>>,
    /// `(entered, release)`: the next fetch signals `entered`, then waits for `release`.
    gate: Mutex<Option<(Arc<Notify>, Arc<Notify>)>>,
}

impl WindowedFetch {
    /// Parks the next fetch until `release` is notified, signalling `entered` once it is parked.
    pub(crate) fn park_next(&self, entered: Arc<Notify>, release: Arc<Notify>) {
        *self.gate.lock().unwrap() = Some((entered, release));
    }

    /// Records `window`, and parks here when the gate is set.
    pub(super) async fn enter(&self, window: Horizon) {
        self.windows.lock().unwrap().push(window);
        let gate = self.gate.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.notify_one();
            release.notified().await;
        }
    }

    /// `sync` with a snapshot narrowed to the events starting inside `window`.
    pub(super) fn keep_inside(sync: ScopeSync<Event>, window: Horizon) -> ScopeSync<Event> {
        let ScopeSync {
            update: SyncUpdate::Snapshot { objects, .. },
            next_cursor,
        } = sync
        else {
            return sync;
        };
        let inside: Vec<Event> = objects
            .into_iter()
            .filter(|event| starts_inside(event, window))
            .collect();
        let present = inside.iter().map(|event| event.id.key().clone()).collect();
        ScopeSync::new(SyncUpdate::snapshot(inside, present), next_cursor)
    }
}

/// Whether `event` (one of the fixtures', which start in UTC) starts inside `window`.
fn starts_inside(event: &Event, window: Horizon) -> bool {
    let CalendarDateTime::Zoned { local, .. } = &event.start else {
        return true;
    };
    let at: UtcDateTime = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:00Z",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute()
    )
    .parse()
    .unwrap();
    window.overlaps(at, at)
}

impl CalendarFake {
    /// A calendar holding `events` that fetches inside the window the engine names.
    pub(crate) fn windowed(events: Vec<Event>) -> Self {
        let fake = Self::with_events(events);
        Self {
            caps: fake.caps.with_windowed_events(),
            windowed: Arc::default(),
            ..fake
        }
    }

    /// A shared handle to how this provider fetches inside a window.
    pub(crate) fn windowed_fetch(&self) -> Arc<WindowedFetch> {
        Arc::clone(&self.windowed)
    }
}
