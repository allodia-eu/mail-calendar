//! Observer adapters: [`ObserverBridge`] wraps a foreign [`Observer`] as an
//! [`AppObserver`], and [`DebouncedObserver`] rate-limits the two surfaces a sync burst
//! drives ([`AppSurface::MailboxList`] and [`AppSurface::SyncProgress`]) to one
//! notification each per [`DEBOUNCE_MS`]. The list also [leads](takes_leading_edge): out of a
//! quiet period it is forwarded at once rather than at the window's end, because a user
//! action produces exactly one of those and has nothing to coalesce with. Every other surface
//! passes through immediately. Split out of `convert.rs` to keep each file under the 500-line
//! limit; no FFI macros live here.

use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use mailcal_app::{AppObserver, Surface as AppSurface};

use crate::Observer;

/// Adapts a foreign [`Observer`] to the app's [`AppObserver`], converting the surface.
pub(crate) struct ObserverBridge {
    pub(crate) foreign: Box<dyn Observer>,
}

impl AppObserver for ObserverBridge {
    fn surface_changed(&self, surface: AppSurface) {
        self.foreign.surface_changed(surface.into());
    }
}

/// How long a [`DebouncedObserver`] window lasts: one notification per surface per window,
/// plus the leading one for the surfaces that [take it](takes_leading_edge).
///
/// 250 ms keeps the UI feeling live (~4 renders/s while a sync streams) while eliminating the
/// hundreds-per-second churn that caused list flickering on Android and Windows. It bounds a
/// burst, and deliberately does not delay the signal that starts one: a folder click
/// produces exactly one signal, and making it wait a window is a wait on every navigation.
const DEBOUNCE_MS: u64 = 250;

/// Returns `true` for surfaces a sync burst drives, which are coalesced rather than
/// forwarded on every commit.
///
/// **Both** of them, and the pairing is the point. A streamed pass commits one message at a
/// time, and each commit signals `MailboxList` (the spliced list) and then `SyncProgress`
/// (the counts). Debouncing only the first achieved nothing: the `SyncProgress` that
/// followed it was undebounced, so it ran [`DebouncedObserver::flush_pending`] and delivered
/// the `MailboxList` signal it had just queued: a full-snapshot pull and list reconcile per
/// message, on every client, plus the progress signal beside it.
///
/// `SyncProgress` was exempted on the grounds that it drives only a lightweight bar and that
/// delaying it would hide the bar for a download that finishes inside one window. Neither
/// holds. A signal carries no payload (the host pulls the current state when it arrives) so
/// a coalesced burst always reports where the sync actually is, and the trailing fire after
/// the last signal always delivers the final state. A download that finished in 100 ms then
/// simply never raises the bar, which is what it should do; today it flashes one.
fn is_debounced(surface: AppSurface) -> bool {
    matches!(surface, AppSurface::MailboxList | AppSurface::SyncProgress)
}

/// Returns `true` for a debounced surface whose first signal out of a quiet period is worth
/// forwarding straight away rather than at the window's end.
///
/// Only the list. It is what a user action changes: a folder click rebuilds once and signals
/// once, with nothing to coalesce it with: so holding that signal put the window in front of
/// every navigation on every platform.
///
/// `SyncProgress` deliberately does not: the bar is *supposed* to stay down for a download
/// that finishes inside one window, and a leading edge would raise it and take it away again
/// on every short pass (`docs/sync-progress.md`). Late is right for a bar; late is wrong for
/// the list.
fn takes_leading_edge(surface: AppSurface) -> bool {
    matches!(surface, AppSurface::MailboxList)
}

#[derive(Debug, Default)]
struct DebouncedState {
    /// The debounced surfaces awaiting delivery, in the order they were first signalled this
    /// window; at most one entry each, which is what makes a burst one notification.
    pending: Vec<AppSurface>,
    /// When the earliest pending surface's window closes, and so when the drain thread
    /// should next look.
    deadline: Option<Instant>,
    /// The open rate-limit window per surface: the "each" in one notification per surface
    /// per [`DEBOUNCE_MS`]. Per surface rather than shared, so that a sync ticking the
    /// progress bar cannot hold the window the list's leading edge tests: with one window
    /// between them, a folder opened mid-sync waited out whatever was left of it.
    windows: Vec<(AppSurface, Instant)>,
}

impl DebouncedState {
    /// Records `surface` and answers whether the caller should forward it **now**.
    ///
    /// A surface that [takes a leading edge](takes_leading_edge) and arrives out of a quiet
    /// period is forwarded immediately; everything else is queued for its window's end, which
    /// is what collapses a streamed sync to one notification per surface.
    fn mark(&mut self, surface: AppSurface, now: Instant) -> bool {
        if self.window_end(surface).is_none_or(|ends| ends <= now) {
            self.open_window(surface, now);
            if takes_leading_edge(surface) {
                return true;
            }
        }
        if !self.pending.contains(&surface) {
            self.pending.push(surface);
        }
        // Always the window's end, never `now + DEBOUNCE_MS`. Pushing the deadline forward on
        // every signal meant a pass that commits continuously never reached it at all: the
        // list was not rebuilt until the download stopped for a whole window.
        self.refresh_deadline();
        false
    }

    /// Puts the deadline on the earliest window a queued surface is waiting for.
    fn refresh_deadline(&mut self) {
        self.deadline = self
            .pending
            .iter()
            .filter_map(|surface| self.window_end(*surface))
            .min();
    }

    /// The end of `surface`'s open window, if it has one.
    fn window_end(&self, surface: AppSurface) -> Option<Instant> {
        self.windows
            .iter()
            .find(|(open, _)| *open == surface)
            .map(|(_, ends)| *ends)
    }

    /// Starts `surface`'s next window, replacing any window it already had.
    fn open_window(&mut self, surface: AppSurface, now: Instant) {
        let ends = now + Duration::from_millis(DEBOUNCE_MS);
        if let Some((_, open)) = self.windows.iter_mut().find(|(open, _)| *open == surface) {
            *open = ends;
        } else {
            self.windows.push((surface, ends));
        }
    }

    /// Takes the queued surfaces whose window has closed, opening each one's next window, and
    /// leaves the deadline on the earliest still waiting.
    fn take_due(&mut self, now: Instant) -> Vec<AppSurface> {
        let due: Vec<AppSurface> = self
            .pending
            .iter()
            .copied()
            .filter(|surface| self.window_end(*surface).is_none_or(|ends| ends <= now))
            .collect();
        self.pending.retain(|surface| !due.contains(surface));
        for surface in &due {
            self.open_window(*surface, now);
        }
        self.refresh_deadline();
        due
    }

    /// Takes everything queued whatever its window, clearing the deadline.
    fn take(&mut self) -> Vec<AppSurface> {
        self.deadline = None;
        std::mem::take(&mut self.pending)
    }
}

/// Wraps an [`ObserverBridge`] and rate-limits the debounced surfaces: everything arriving
/// while a surface's window is open collapses into one notification at its end, so a streamed
/// sync becomes a few re-renders instead of hundreds. A surface that
/// [leads](takes_leading_edge) is forwarded at once when it arrives out of a quiet period, so
/// the window never lands in front of a user action. All other surfaces are forwarded
/// immediately and also flush any pending debounced signal so the host always sees a
/// consistent state.
pub(crate) struct DebouncedObserver {
    inner: Arc<ObserverBridge>,
    state: Arc<(Mutex<DebouncedState>, Condvar)>,
}

impl DebouncedObserver {
    /// Builds a [`DebouncedObserver`] and spawns the background drain thread.
    pub(crate) fn new(bridge: ObserverBridge) -> Self {
        let inner = Arc::new(bridge);
        let state: Arc<(Mutex<DebouncedState>, Condvar)> =
            Arc::new((Mutex::new(DebouncedState::default()), Condvar::new()));
        let drain_inner = Arc::clone(&inner);
        let drain_state = Arc::clone(&state);
        thread::Builder::new()
            .name("mailcal-observer-debounce".to_owned())
            .spawn(move || Self::drain_loop(&drain_inner, &drain_state))
            .expect("debounce drain thread spawns");
        Self { inner, state }
    }

    /// The background thread: sleeps until the next deadline, then fires the pending signal.
    fn drain_loop(inner: &ObserverBridge, state: &(Mutex<DebouncedState>, Condvar)) {
        let (lock, cvar) = state;
        loop {
            // Wait until there is a deadline to fire.
            let deadline = {
                let guard = lock.lock().expect("debounce mutex not poisoned");
                let guard = cvar
                    .wait_while(guard, |s| s.deadline.is_none())
                    .expect("debounce condvar not poisoned");
                guard.deadline.expect("deadline is set when we wake")
            };

            // Sleep until the deadline (or immediately if it already passed).
            if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                thread::sleep(remaining);
            }

            // Fire only if the deadline hasn't been pushed forward again while we slept.
            let should_fire = {
                let mut guard = lock.lock().expect("debounce mutex not poisoned");
                if guard
                    .deadline
                    .is_some_and(|d| d.checked_duration_since(Instant::now()).is_some())
                {
                    // Deadline still in the future; go back and sleep for the remainder.
                    Vec::new()
                } else {
                    // Draining opens each surface's next window, so a pass that keeps
                    // committing repaints once per window rather than on every commit.
                    guard.take_due(Instant::now())
                }
            };

            for surface in should_fire {
                inner.surface_changed(surface);
            }
        }
    }

    /// Immediately fires every queued debounced signal. Called before forwarding any
    /// non-debounced signal so the host always sees a consistent state.
    fn flush_pending(&self) {
        let (lock, _) = &*self.state;
        let pending = {
            let mut guard = lock.lock().expect("debounce mutex not poisoned");
            guard.take()
        };
        for surface in pending {
            self.inner.surface_changed(surface);
        }
    }
}

impl AppObserver for DebouncedObserver {
    fn surface_changed(&self, surface: AppSurface) {
        if is_debounced(surface) {
            let (lock, cvar) = &*self.state;
            let forward_now = {
                let mut guard = lock.lock().expect("debounce mutex not poisoned");
                let forward_now = guard.mark(surface, Instant::now());
                if !forward_now {
                    cvar.notify_one();
                }
                forward_now
            };
            // Outside the lock: the host renders on this call, and holding the mutex across
            // it would put every other signalling task behind the render.
            if forward_now {
                self.inner.surface_changed(surface);
            }
        } else {
            // Non-debounced surface: flush anything queued first (so the host's snapshot is
            // consistent), then forward immediately.
            self.flush_pending();
            self.inner.surface_changed(surface);
        }
    }
}

#[cfg(test)]
#[path = "observer_debounce_tests.rs"]
mod debounce_tests;
