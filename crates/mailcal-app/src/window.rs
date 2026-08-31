//! The mailbox-list paging window: how many rows are shown, and how deep the store read that
//! backs them goes. Factored out of `lib.rs` (which holds the runtime struct) so the sizing
//! policy (a single responsibility) lives on its own and the struct file stays under the
//! length cap. The tuning constants themselves stay in the crate root beside the struct.

use engine_api::Provider;

use crate::{App, MIN_LOAD_WINDOW, PAGE, WINDOW_BUCKET, WINDOW_ROW_FACTOR};

impl<P: Provider> App<P> {
    /// Resets the visible mailbox-list window to the first [`PAGE`]; called on any
    /// navigation that changes which list is shown, so it always opens at the top.
    pub(crate) fn reset_window(&self) {
        *self.visible_limit.lock().expect("window mutex poisoned") = PAGE;
    }

    /// Grows the visible window by one [`PAGE`]: the host's "scrolled near the end, show
    /// more" step. It can grow past the row count harmlessly; the projection just returns
    /// every row (and reports `total`), so the host stops asking once it has them all.
    pub(crate) fn grow_window(&self) {
        let mut limit = self.visible_limit.lock().expect("window mutex poisoned");
        *limit = limit.saturating_add(PAGE);
    }

    /// The current visible window size, for [`rebuild_snapshot`](Self::rebuild_snapshot).
    pub(crate) fn visible_limit(&self) -> usize {
        *self.visible_limit.lock().expect("window mutex poisoned")
    }

    /// How many of each account's newest messages a snapshot reads from the store; the
    /// message-load window. Scales with the visible row limit (so
    /// [`Intent::ShowMore`](crate::Intent::ShowMore) loads deeper) over a floor of
    /// [`MIN_LOAD_WINDOW`], and stays far below a large mailbox's total so boot reads hundreds
    /// of rows, not thousands. The threaded view still pulls a shown conversation's
    /// out-of-window members from the store's thread index, so this window bounds only which
    /// conversations *appear*, never how much of one the user can read.
    pub(crate) fn load_window(&self) -> usize {
        let needed = self
            .visible_limit()
            .saturating_mul(WINDOW_ROW_FACTOR)
            .max(MIN_LOAD_WINDOW);
        if needed <= MIN_LOAD_WINDOW {
            // Keep boot's first window at the small floor: no bucketing, so it stays fast.
            return MIN_LOAD_WINDOW;
        }
        // Round up to the bucket so consecutive ShowMore steps land on the same window and reuse
        // the cache, reloading only when scrolling crosses a bucket boundary.
        needed.div_ceil(WINDOW_BUCKET).saturating_mul(WINDOW_BUCKET)
    }
}
