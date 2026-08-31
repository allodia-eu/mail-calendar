//! Message-list grouping (flat vs threaded) persistence.
//!
//! Unlike the other settings, the runtime grouping lives in `App::view_mode` (read on every
//! snapshot rebuild), so this module doesn't own a state struct; it only maps that runtime
//! [`ViewMode`] to/from the persisted [`mailcal_account::MessageGrouping`] and does the
//! read-modify-write that keeps the sibling preferences intact. Formerly the grouping was a
//! runtime-only toggle; persisting it (default Threaded) is what lets the choice survive a
//! restart and be edited in the Settings screen. A separate module keeps `lib.rs` from growing.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{MessageGrouping, load_preferences, save_preferences};
use mailcal_viewmodel::ViewMode;

use crate::App;

/// Maps the persisted grouping to the runtime view mode.
fn to_mode(grouping: MessageGrouping) -> ViewMode {
    match grouping {
        MessageGrouping::Flat => ViewMode::Flat,
        MessageGrouping::Threaded => ViewMode::Threaded,
    }
}

/// Maps the runtime view mode to the persisted grouping.
fn to_grouping(mode: ViewMode) -> MessageGrouping {
    match mode {
        ViewMode::Flat => MessageGrouping::Flat,
        ViewMode::Threaded => MessageGrouping::Threaded,
    }
}

/// Loads the persisted grouping as a [`ViewMode`], defaulting to Threaded (the product default)
/// when there is no preferences file (the demo/tests) or the field is absent/unreadable.
pub(crate) fn load_view_mode(prefs_path: Option<&PathBuf>) -> ViewMode {
    to_mode(
        prefs_path
            .map(|path| load_preferences(path).message_grouping)
            .unwrap_or_default(),
    )
}

impl<P: Provider> App<P> {
    /// The current message-list grouping; pulled by the host after a
    /// [`crate::Surface::Settings`] signal to render the grouping control in the settings screen.
    #[must_use]
    pub fn view_mode(&self) -> ViewMode {
        *self.view_mode.lock().expect("view-mode mutex poisoned")
    }

    /// Persists `mode` as the grouping preference, read-modify-write so the sibling display-zone /
    /// sync / quote-style preferences in the same file are preserved. Best effort; a no-op
    /// without a preferences path (the demo/tests).
    pub(crate) fn persist_view_mode(&self, mode: ViewMode) {
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.message_grouping = to_grouping(mode);
            let _ = save_preferences(path, &prefs);
        }
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::MessageGrouping;
    use mailcal_viewmodel::ViewMode;

    use super::{to_grouping, to_mode};

    #[test]
    fn grouping_and_mode_round_trip_both_ways() {
        for mode in [ViewMode::Flat, ViewMode::Threaded] {
            assert_eq!(to_mode(to_grouping(mode)), mode);
        }
        for grouping in [MessageGrouping::Flat, MessageGrouping::Threaded] {
            assert_eq!(to_grouping(to_mode(grouping)), grouping);
        }
    }
}
