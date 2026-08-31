//! The per-direction swipe-action setting: what a leftward / rightward swipe across a message
//! row does (Trash, Archive, or Star). A persisted app-level preference the host reads to bind
//! its swipe gestures and shows in app settings. Like the quote-style and display-zone settings
//! ([`crate::quote_settings`], [`crate::timezone`]) it lives in the shared preferences file; this
//! small state holds the loaded values and writes them back read-modify-write so the sibling
//! preferences are preserved. A second `impl App` block keeps `lib.rs` under the 500-line limit.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{SwipeAction, load_preferences, save_preferences};
use mailcal_viewmodel::{SwipeActionKind, SwipeDirection, SwipeSettings};

use crate::{App, Surface};

/// The loaded per-direction swipe actions and where to persist them.
pub(crate) struct SwipeSettingsState {
    left: SwipeAction,
    right: SwipeAction,
    prefs_path: Option<PathBuf>,
}

impl SwipeSettingsState {
    /// Loads the persisted actions from the preferences file (both Delete when absent or
    /// unreadable: the behaviour before the setting existed).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        Self {
            left: prefs.swipe_left,
            right: prefs.swipe_right,
            prefs_path,
        }
    }

    fn get(&self) -> (SwipeAction, SwipeAction) {
        (self.left, self.right)
    }

    /// Stores one direction's action and persists it. Read-modify-write so the sibling
    /// display-zone / sync-depth / quote-style / per-account preferences survive.
    fn set(&mut self, direction: SwipeDirection, action: SwipeAction) {
        match direction {
            SwipeDirection::Left => self.left = action,
            SwipeDirection::Right => self.right = action,
        }
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.swipe_left = self.left;
            prefs.swipe_right = self.right;
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The per-direction swipe actions (pulled after a [`Surface::Settings`] signal). The host
    /// binds its row swipes to these and renders them in the settings screen.
    #[must_use]
    pub fn swipe_settings(&self) -> SwipeSettings {
        let (left, right) = self
            .swipe_settings
            .lock()
            .expect("swipe-settings mutex poisoned")
            .get();
        SwipeSettings {
            left: kind(left),
            right: kind(right),
        }
    }

    /// Sets and persists what one swipe direction does, then signals [`Surface::Settings`] so
    /// the host re-pulls.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly (this one just locks,
    // sets, and signals).
    #[allow(clippy::unused_async)]
    pub async fn set_swipe_action(&self, direction: SwipeDirection, action: SwipeActionKind) {
        self.swipe_settings
            .lock()
            .expect("swipe-settings mutex poisoned")
            .set(direction, to_action(action));
        self.observer.surface_changed(Surface::Settings);
    }
}

/// Maps the persisted action to the host-facing kind.
fn kind(action: SwipeAction) -> SwipeActionKind {
    match action {
        SwipeAction::Delete => SwipeActionKind::Delete,
        SwipeAction::Archive => SwipeActionKind::Archive,
        SwipeAction::Star => SwipeActionKind::Star,
    }
}

/// Maps the host-facing kind back to the persisted action.
fn to_action(kind: SwipeActionKind) -> SwipeAction {
    match kind {
        SwipeActionKind::Delete => SwipeAction::Delete,
        SwipeActionKind::Archive => SwipeAction::Archive,
        SwipeActionKind::Star => SwipeAction::Star,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{MessageGrouping, SwipeAction, load_preferences, save_preferences};
    use mailcal_viewmodel::SwipeDirection;

    use super::SwipeSettingsState;

    #[test]
    fn each_direction_persists_independently_without_clobbering_siblings() {
        let dir = std::env::temp_dir().join("mailcal-swipe-actions-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        // Seed a sibling preference the swipe write must preserve.
        let seeded = mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();

        // Both default to Delete, then each direction is set on its own.
        let mut state = SwipeSettingsState::new(Some(path.clone()));
        assert_eq!(state.get(), (SwipeAction::Delete, SwipeAction::Delete));
        state.set(SwipeDirection::Left, SwipeAction::Archive);
        // Setting Left leaves Right alone.
        assert_eq!(state.get(), (SwipeAction::Archive, SwipeAction::Delete));
        state.set(SwipeDirection::Right, SwipeAction::Star);

        // A fresh state reads back both persisted choices, and the siblings survived.
        assert_eq!(
            SwipeSettingsState::new(Some(path.clone())).get(),
            (SwipeAction::Archive, SwipeAction::Star)
        );
        let on_disk = load_preferences(&path);
        assert_eq!(
            on_disk.display_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(on_disk.message_grouping, MessageGrouping::Flat);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
