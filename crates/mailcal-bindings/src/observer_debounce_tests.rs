//! The rate limiter's contract: a signal a user action produced reaches the host at once,
//! and a streamed sync collapses to one notification per surface per window.
//!
//! Its own file because [`observer`](super) is close to the 500-line limit and these are
//! timing tests, which are the part that grows.

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use mailcal_app::{AppObserver, Surface as AppSurface};

use super::{DEBOUNCE_MS, DebouncedObserver, ObserverBridge};
use crate::{Observer, Surface};

// Converts the FFI `Surface` back to `AppSurface` for test channel recording.
fn to_app(surface: Surface) -> AppSurface {
    match surface {
        Surface::MailboxList => AppSurface::MailboxList,
        Surface::Calendar => AppSurface::Calendar,
        Surface::Settings => AppSurface::Settings,
        Surface::Reading => AppSurface::Reading,
        Surface::Sending => AppSurface::Sending,
        Surface::SyncProgress => AppSurface::SyncProgress,
        Surface::Connectivity => AppSurface::Connectivity,
        Surface::CalendarStatus => AppSurface::CalendarStatus,
        Surface::Contacts => AppSurface::Contacts,
        Surface::InvitationReply => AppSurface::InvitationReply,
        Surface::UnfiledCopy => AppSurface::UnfiledCopy,
    }
}

struct ChanObserver(mpsc::Sender<AppSurface>);
impl Observer for ChanObserver {
    fn surface_changed(&self, surface: Surface) {
        let _ = self.0.send(to_app(surface));
    }
}

fn make_obs() -> (DebouncedObserver, mpsc::Receiver<AppSurface>) {
    let (tx, rx) = mpsc::channel();
    let obs = DebouncedObserver::new(ObserverBridge {
        foreign: Box::new(ChanObserver(tx)),
    });
    (obs, rx)
}

#[test]
fn a_burst_delivers_a_leading_signal_and_one_trailing_one() {
    let (obs, rx) = make_obs();
    // Fire 20 MailboxList signals in rapid succession.
    let sent = Instant::now();
    for _ in 0..20 {
        obs.surface_changed(AppSurface::MailboxList);
    }
    // The first goes straight through, so whatever produced it is on screen now.
    let first = rx
        .recv_timeout(Duration::from_millis(DEBOUNCE_MS * 4))
        .expect("the leading MailboxList signal");
    assert!(matches!(first, AppSurface::MailboxList));
    assert!(
        sent.elapsed() < Duration::from_millis(DEBOUNCE_MS / 2),
        "the leading signal waited {:?}",
        sent.elapsed()
    );
    // The other nineteen collapse into one trailing notification carrying the final state;
    // not one per signal, which is the churn this rate limiter exists for.
    let mut trailing = 0;
    while rx
        .recv_timeout(Duration::from_millis(DEBOUNCE_MS * 2))
        .is_ok()
    {
        trailing += 1;
    }
    assert_eq!(
        trailing, 1,
        "nineteen more signals delivered {trailing} time(s)"
    );
}

#[test]
fn non_debounced_surface_is_immediate() {
    let (obs, rx) = make_obs();
    obs.surface_changed(AppSurface::Settings);
    let surface = rx
        .recv_timeout(Duration::from_millis(100))
        .expect("Settings signal delivered immediately");
    assert!(matches!(surface, AppSurface::Settings));
}

#[test]
fn non_debounced_surface_flushes_pending_first() {
    let (obs, rx) = make_obs();
    // Queue a debounced surface, then immediately fire a non-debounced one.
    obs.surface_changed(AppSurface::MailboxList);
    obs.surface_changed(AppSurface::Settings);
    // flush_pending fires MailboxList first, then Settings immediately after.
    let first = rx
        .recv_timeout(Duration::from_millis(100))
        .expect("first signal");
    let second = rx
        .recv_timeout(Duration::from_millis(100))
        .expect("second signal");
    assert!(
        matches!(first, AppSurface::MailboxList),
        "pending flush fires first"
    );
    assert!(
        matches!(second, AppSurface::Settings),
        "non-debounced fires second"
    );
}

#[test]
fn a_gapped_burst_repaints_at_a_bounded_rate() {
    let (obs, rx) = make_obs();
    // Send MailboxList signals with small gaps; each one used to push the deadline forward.
    let gap = Duration::from_millis(DEBOUNCE_MS / 4);
    let obs = Arc::new(obs);
    let obs2 = Arc::clone(&obs);
    thread::spawn(move || {
        for _ in 0..5 {
            obs2.surface_changed(AppSurface::MailboxList);
            thread::sleep(gap);
        }
    });
    // Five signals spanning rather more than one window: the leading one, then at most one
    // notification per window while they keep coming. Bounded, and never one per signal.
    let mut delivered = 0;
    while rx
        .recv_timeout(Duration::from_millis(DEBOUNCE_MS * 2))
        .is_ok()
    {
        delivered += 1;
    }
    assert!(
        (1..=3).contains(&delivered),
        "five signals over ~two windows delivered {delivered} time(s)"
    );
}

#[test]
fn a_lone_signal_reaches_the_host_immediately() {
    // What a folder click is: one rebuild, one signal, nothing else in flight. Coalescing
    // has nothing to coalesce here, so the window must not be spent before the host is
    // told, that delay is on every navigation, on every platform, and it is the whole
    // difference between the list moving under the finger and arriving a beat later.
    let (obs, rx) = make_obs();
    let sent = Instant::now();
    obs.surface_changed(AppSurface::MailboxList);
    let surface = rx
        .recv_timeout(Duration::from_millis(DEBOUNCE_MS * 4))
        .expect("a lone MailboxList signal is delivered");
    let waited = sent.elapsed();
    assert!(matches!(surface, AppSurface::MailboxList));
    assert!(
        waited < Duration::from_millis(DEBOUNCE_MS / 2),
        "a lone signal waited {waited:?}, which is the debounce window rather than the \
         leading edge"
    );
}

#[test]
fn a_burst_that_never_pauses_still_repaints() {
    // A streamed sync commits continuously, and every commit used to push the deadline
    // forward: so the list was not rebuilt until the download stopped for a full window.
    // On a big folder that is the whole download spent on a list that never moves.
    let (obs, rx) = make_obs();
    let obs = Arc::new(obs);
    let feeder = Arc::clone(&obs);
    let burst = thread::spawn(move || {
        let until = Instant::now() + Duration::from_millis(DEBOUNCE_MS * 3);
        while Instant::now() < until {
            feeder.surface_changed(AppSurface::MailboxList);
            thread::sleep(Duration::from_millis(10));
        }
    });
    let mut delivered = 0;
    let deadline = Instant::now() + Duration::from_millis(DEBOUNCE_MS * 3);
    while Instant::now() < deadline {
        if rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)).is_ok() {
            delivered += 1;
        }
    }
    burst.join().expect("the feeder thread finishes");
    assert!(
        delivered >= 2,
        "a burst spanning three windows repainted {delivered} time(s): the deadline is \
         being pushed forward instead of capped"
    );
}

#[test]
fn a_sync_progress_burst_is_coalesced_too() {
    // The regression this pairing exists for. A streamed pass commits one message at a
    // time and signals BOTH surfaces per commit; leaving SyncProgress undebounced meant it
    // flushed the MailboxList queued the instant before, so neither was ever coalesced;
    // a full-snapshot pull and list reconcile per message, thousands of times, on the
    // thread the client renders on.
    let (obs, rx) = make_obs();
    let sent = Instant::now();
    for _ in 0..20 {
        obs.surface_changed(AppSurface::SyncProgress);
    }
    // One, and not before the window is out. The bar takes no leading edge because it is meant
    // to stay down for a download that finishes inside a window rather than raise and drop on
    // every short pass (docs/sync-progress.md): late is right for a bar, and wrong for a list.
    let mut delivered = 0;
    while rx
        .recv_timeout(Duration::from_millis(DEBOUNCE_MS * 2))
        .is_ok()
    {
        delivered += 1;
    }
    assert!(
        sent.elapsed() >= Duration::from_millis(DEBOUNCE_MS),
        "the bar went up after {:?}, inside its own window, that is the flash",
        sent.elapsed()
    );
    assert_eq!(delivered, 1, "twenty signals delivered {delivered} time(s)");
}

#[test]
fn a_commit_signalling_both_surfaces_delivers_each_on_its_own_window() {
    // What a streamed sync actually does, twenty messages of it. Before, this delivered
    // forty signals; the interleaving was the defect, not the volume of either alone. Each
    // surface keeps its own window, so each gets its leading signal rather than the one that
    // happened to be signalled first taking the other's.
    let (obs, rx) = make_obs();
    for _ in 0..20 {
        obs.surface_changed(AppSurface::MailboxList);
        obs.surface_changed(AppSurface::SyncProgress);
    }
    let mut delivered = Vec::new();
    while let Ok(surface) = rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS * 4)) {
        delivered.push(surface);
    }
    assert_eq!(
        delivered
            .iter()
            .filter(|s| **s == AppSurface::MailboxList)
            .count(),
        2,
        "one leading and one trailing MailboxList, got {delivered:?}"
    );
    assert_eq!(
        delivered
            .iter()
            .filter(|s| **s == AppSurface::SyncProgress)
            .count(),
        1,
        "the bar takes no leading edge, so one trailing only, got {delivered:?}"
    );
}

#[test]
fn the_final_state_is_always_delivered_after_the_burst_stops() {
    // Why coalescing SyncProgress cannot strand the bar: the signal carries no payload, so
    // the trailing fire after the last commit is the host reading wherever the sync ended
    // up. A pass that finishes inside one window then never raises a bar at all, which is
    // the right answer for a download that is already over.
    let (obs, rx) = make_obs();
    obs.surface_changed(AppSurface::SyncProgress);
    obs.surface_changed(AppSurface::SyncProgress);
    assert_eq!(
        rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS * 4)),
        Ok(AppSurface::SyncProgress),
        "the window's trailing fire must still reach the host"
    );
}
