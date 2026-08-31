//! A snapshot a host pulls, and the signal that tells it to: as one thing.
//!
//! The hazard this closes is an ordering one, and it is invisible in review. A host learns a
//! surface changed from [`AppObserver::surface_changed`] and then *pulls* the snapshot; if the
//! signal goes out before the snapshot is written, the host reads the previous one and paints
//! stale. Nothing then corrects it; there is no second signal coming, because as far as the core
//! is concerned it already told the host.
//!
//! While the field and the observer were separate, that ordering held only by everyone
//! remembering it, and the write was sometimes in a different file from the signal
//! (`live_mailbox.rs` rebuilt the mailbox snapshot; `sync_progress.rs` signalled it). Here the
//! observer is *inside* the cell, so [`publish`](Surfaced::publish) is the only way to store a
//! value and it always signals afterwards. There is no method that writes without signalling and
//! none that signals a value it has not stored: the pull, [`get`](Surfaced::get), signals
//! nothing at all.
//!
//! [`resignal`](Surfaced::resignal) is the deliberate exception, and it is safe by construction:
//! it sends the signal for a value that is *already* published, so there is nothing for it to be
//! ahead of. Two callers need it: a cold offline launch re-announcing an already-primed list, and
//! the calendar preferences, whose pull (`calendar_page`) recomputes from state written before the
//! call rather than from the stored snapshot.
//!
//! `scripts/ci/check_surface_publish.py` keeps the door shut: it fails if a published surface is
//! signalled anywhere but here, or if one of these fields is declared as a bare `Mutex`.

use std::sync::{Arc, Mutex};

use crate::{AppObserver, Surface};

/// A snapshot the host pulls, its surface, and the observer to announce it on.
///
/// `T` is cloned out on every pull, so it is one of the view-model snapshot types; small,
/// owned, and cheap to hand across the FFI boundary.
pub(crate) struct Surfaced<T> {
    value: Mutex<T>,
    surface: Surface,
    observer: Arc<dyn AppObserver>,
}

impl<T> core::fmt::Debug for Surfaced<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The value may hold the user's mail; the surface alone is enough to identify the cell.
        f.debug_struct("Surfaced")
            .field("surface", &self.surface)
            .finish_non_exhaustive()
    }
}

impl<T: Clone + Default> Surfaced<T> {
    /// An unpublished cell holding `T::default()`, bound to `surface` and `observer`.
    pub(crate) fn new(surface: Surface, observer: Arc<dyn AppObserver>) -> Self {
        Self {
            value: Mutex::new(T::default()),
            surface,
            observer,
        }
    }
}

impl<T: Clone> Surfaced<T> {
    /// Stores `value` and then announces it. The store happens first, always: a host that pulls
    /// the instant it is signalled gets this value and not the one before it.
    pub(crate) fn publish(&self, value: T) {
        *self.value.lock().expect("surfaced mutex poisoned") = value;
        self.observer.surface_changed(self.surface);
    }

    /// The current value, for the host's pull.
    pub(crate) fn get(&self) -> T {
        self.value.lock().expect("surfaced mutex poisoned").clone()
    }

    /// Announces what is **already** published, without writing.
    ///
    /// For the two cases where the thing that went stale is not this snapshot: a cold offline
    /// launch whose primed list was signalled before the host had wired its observer, and a
    /// preference change whose pull recomputes from state written before this call. It cannot
    /// reintroduce the ordering bug; there is no new value for the signal to run ahead of.
    pub(crate) fn resignal(&self) {
        self.observer.surface_changed(self.surface);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::Surfaced;
    use crate::{AppObserver, Surface};

    /// An observer that pulls the cell the moment it is signalled, which is exactly what a host
    /// does, and the only way to catch a signal that ran ahead of its write.
    struct PullsOnSignal {
        cell: Mutex<Option<Arc<Surfaced<String>>>>,
        seen: Mutex<Vec<String>>,
        surfaces: Mutex<Vec<Surface>>,
    }

    impl PullsOnSignal {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                cell: Mutex::new(None),
                seen: Mutex::new(Vec::new()),
                surfaces: Mutex::new(Vec::new()),
            })
        }
    }

    impl AppObserver for PullsOnSignal {
        fn surface_changed(&self, surface: Surface) {
            self.surfaces.lock().unwrap().push(surface);
            let cell = self.cell.lock().unwrap().clone();
            if let Some(cell) = cell {
                self.seen.lock().unwrap().push(cell.get());
            }
        }
    }

    fn wired() -> (Arc<Surfaced<String>>, Arc<PullsOnSignal>) {
        let observer = PullsOnSignal::new();
        let cell = Arc::new(Surfaced {
            value: Mutex::new(String::new()),
            surface: Surface::MailboxList,
            observer: observer.clone(),
        });
        *observer.cell.lock().unwrap() = Some(Arc::clone(&cell));
        (cell, observer)
    }

    #[test]
    fn a_published_value_is_readable_at_the_moment_it_is_signalled() {
        // The whole point. A host that pulls inside `surface_changed` must see the new value; if
        // the signal were sent first, this would read the empty string.
        let (cell, observer) = wired();
        cell.publish("first".to_owned());
        cell.publish("second".to_owned());
        assert_eq!(
            *observer.seen.lock().unwrap(),
            vec!["first".to_owned(), "second".to_owned()]
        );
    }

    #[test]
    fn every_signal_carries_the_cells_own_surface() {
        let (cell, observer) = wired();
        cell.publish("x".to_owned());
        cell.resignal();
        assert_eq!(
            *observer.surfaces.lock().unwrap(),
            vec![Surface::MailboxList; 2],
            "a cell cannot be made to announce a surface it does not own"
        );
    }

    #[test]
    fn resignal_repeats_the_published_value_and_writes_nothing() {
        let (cell, observer) = wired();
        cell.publish("only".to_owned());
        cell.resignal();
        assert_eq!(cell.get(), "only");
        assert_eq!(
            *observer.seen.lock().unwrap(),
            vec!["only".to_owned(), "only".to_owned()],
            "a re-signal repeats what is published rather than announcing something new"
        );
    }

    #[test]
    fn a_pull_never_signals() {
        // `get` is the host's read path and runs on every pull; if it signalled, a host that
        // reads on signal would loop forever.
        let (cell, observer) = wired();
        cell.publish("x".to_owned());
        let before = observer.surfaces.lock().unwrap().len();
        for _ in 0..5 {
            let _ = cell.get();
        }
        assert_eq!(observer.surfaces.lock().unwrap().len(), before);
    }

    #[test]
    fn concurrent_publishers_each_signal_exactly_once() {
        let (cell, observer) = wired();
        let count = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for index in 0..8 {
                let (cell, count) = (Arc::clone(&cell), Arc::clone(&count));
                scope.spawn(move || {
                    cell.publish(format!("v{index}"));
                    count.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        assert_eq!(count.load(Ordering::SeqCst), 8);
        assert_eq!(observer.surfaces.lock().unwrap().len(), 8);
        // Whichever won, the last value seen by the observer is a real published one, never a
        // half-written or empty cell.
        assert!(
            observer
                .seen
                .lock()
                .unwrap()
                .iter()
                .all(|seen| seen.starts_with('v'))
        );
    }
}
