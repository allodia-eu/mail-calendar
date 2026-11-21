//! Settings → Advanced's reset of the local database: clear every account's cache, download
//! mail again, then compact the file. One runs at a time, and the page says what it came to.
//!
//! The re-sync's reconciling snapshot hard-deletes every message now outside the sync-depth
//! window, but SQLite keeps the file at its high-water mark and reuses the freed pages, so the
//! database would stay at its pre-reset peak. It is compacted **after** the re-sync has settled,
//! once those deletions are committed. Compacting rewrites the whole file, so it runs once here
//! and never on the sync path.

use engine_api::Provider;
use mailcal_viewmodel::{ResetOutcome, ResetStatus};

use crate::{App, Surface, sync::RefreshProgress};

/// Whether a reset runs, and what the last one came to.
#[derive(Debug, Default)]
pub(crate) struct ResetState {
    status: ResetStatus,
}

impl<P: Provider> App<P> {
    /// Settings → Advanced's reset, as its button draws it (pulled after a
    /// [`Surface::Settings`] signal). Read from memory, so a UI thread may ask it.
    #[must_use]
    pub fn reset_status(&self) -> ResetStatus {
        self.reset.lock().expect("reset mutex poisoned").status
    }

    /// Runs [`Intent::ResetDatabase`](crate::Intent::ResetDatabase).
    pub(crate) async fn reset_database(&self) {
        {
            let mut state = self.reset.lock().expect("reset mutex poisoned");
            if state.status.running {
                log::info!("reset: already running; this request is dropped");
                return;
            }
            state.status = ResetStatus {
                running: true,
                outcome: None,
            };
        }
        self.observer.surface_changed(Surface::Settings);

        let outcome = if let Err(err) = self.engine.reset().await {
            log::warn!("reset: the local database could not be cleared: {err}");
            ResetOutcome::Failed
        } else {
            // An explicit, awaited re-download: the progress bar shows although the mail from
            // before the reset is still on screen.
            self.refresh_mail(RefreshProgress::Awaited).await;
            // The mail is already as it should be; a file left larger than it needs to be is
            // not worth telling the person about.
            if let Err(err) = self.engine.vacuum().await {
                log::warn!("reset: the database could not be compacted: {err}");
            }
            ResetOutcome::Done
        };
        log::info!("reset: finished ({outcome:?})");
        self.reset.lock().expect("reset mutex poisoned").status = ResetStatus {
            running: false,
            outcome: Some(outcome),
        };
        self.observer.surface_changed(Surface::Settings);
    }
}

#[cfg(test)]
#[path = "reset_tests.rs"]
mod tests;
