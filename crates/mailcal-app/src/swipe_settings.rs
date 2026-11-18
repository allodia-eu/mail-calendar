//! The per-direction swipe-action setting: what a leftward / rightward swipe across a message
//! row does (Trash, Archive, or Star). A persisted app-level preference the host reads to bind
//! its swipe gestures and shows in app settings. Like the quote-style and display-zone settings
//! ([`crate::quote_settings`], [`crate::timezone`]) it lives in the shared preferences file; this
//! small state holds the loaded values.

use engine_api::Provider;
use mailcal_account::SwipeAction;
use mailcal_viewmodel::{SwipeActionKind, SwipeDirection, SwipeSettings};

use crate::{App, Surface, preferences_file::PreferencesFile};

/// The loaded per-direction swipe actions and where to persist them.
pub(crate) struct SwipeSettingsState {
    left: SwipeAction,
    right: SwipeAction,
    prefs: PreferencesFile,
}

impl SwipeSettingsState {
    /// Loads the persisted actions from the preferences file (both Delete when absent or
    /// unreadable: the behaviour before the setting existed).
    pub(crate) fn new(prefs: PreferencesFile) -> Self {
        let stored = prefs.load();
        Self {
            left: stored.swipe_left,
            right: stored.swipe_right,
            prefs,
        }
    }

    fn get(&self) -> (SwipeAction, SwipeAction) {
        (self.left, self.right)
    }

    /// Stores one direction's action and persists it.
    fn set(&mut self, direction: SwipeDirection, action: SwipeAction) {
        match direction {
            SwipeDirection::Left => self.left = action,
            SwipeDirection::Right => self.right = action,
        }
        let (left, right) = (self.left, self.right);
        self.prefs.edit(move |prefs| {
            prefs.swipe_left = left;
            prefs.swipe_right = right;
        });
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
    use mailcal_account::{MessageGrouping, SwipeAction, load_preferences};
    use mailcal_viewmodel::SwipeDirection;

    use super::SwipeSettingsState;
    use crate::preferences_file::ScratchPreferences;

    #[test]
    fn each_direction_persists_independently_without_clobbering_siblings() {
        // Seed a sibling preference the swipe write must preserve.
        let scratch = ScratchPreferences::seeded(&mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            ..Default::default()
        });

        // Both default to Delete, then each direction is set on its own.
        let mut state = SwipeSettingsState::new(scratch.open());
        assert_eq!(state.get(), (SwipeAction::Delete, SwipeAction::Delete));
        state.set(SwipeDirection::Left, SwipeAction::Archive);
        // Setting Left leaves Right alone.
        assert_eq!(state.get(), (SwipeAction::Archive, SwipeAction::Delete));
        state.set(SwipeDirection::Right, SwipeAction::Star);

        // A fresh state reads back both persisted choices, and the siblings survived.
        assert_eq!(
            SwipeSettingsState::new(scratch.open()).get(),
            (SwipeAction::Archive, SwipeAction::Star)
        );
        let on_disk = load_preferences(scratch.path());
        assert_eq!(
            on_disk.display_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(on_disk.message_grouping, MessageGrouping::Flat);
    }
}
