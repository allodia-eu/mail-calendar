//! The reply/forward quoting settings: the persisted app-level default style, and whether the
//! composer offers a per-message override of it. Like the display-zone and per-account sync
//! settings ([`crate::timezone`], [`crate::sync_settings`]), they live in the shared preferences
//! file; this small state holds the loaded values and writes them back read-modify-write so the
//! sibling preferences are preserved. A second `impl App` block keeps `lib.rs` under the
//! 500-line limit.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{QuoteStyle, load_preferences, save_preferences};
use mailcal_viewmodel::{QuoteSettings, QuoteStyleKind};

use crate::{App, Surface};

/// The loaded quoting settings and where to persist them.
pub(crate) struct QuoteSettingsState {
    style: QuoteStyle,
    per_message: bool,
    prefs_path: Option<PathBuf>,
}

impl QuoteSettingsState {
    /// Loads the persisted settings from the preferences file (an indented quote, with the
    /// per-message picker off, when absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        Self {
            style: prefs.quote_style,
            per_message: prefs.quote_style_per_message,
            prefs_path,
        }
    }

    fn get(&self) -> QuoteSettings {
        QuoteSettings {
            style: kind(self.style),
            per_message: self.per_message,
        }
    }

    /// Stores `style` and persists it. Read-modify-write so the sibling display-zone /
    /// sync-depth / per-account preferences in the same file are preserved.
    fn set_style(&mut self, style: QuoteStyle) {
        self.style = style;
        self.persist(|prefs| prefs.quote_style = style);
    }

    /// Stores whether the composer offers a per-message override, and persists it.
    fn set_per_message(&mut self, per_message: bool) {
        self.per_message = per_message;
        self.persist(|prefs| prefs.quote_style_per_message = per_message);
    }

    fn persist(&self, edit: impl FnOnce(&mut mailcal_account::Preferences)) {
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            edit(&mut prefs);
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The reply/forward quoting settings (pulled after a [`Surface::Settings`] signal): the
    /// default style a new reply's composer is seeded with, and whether the composer offers the
    /// user a per-message override of it (off by default).
    #[must_use]
    pub fn quote_settings(&self) -> QuoteSettings {
        self.quote_settings
            .lock()
            .expect("quote-settings mutex poisoned")
            .get()
    }

    /// Sets and persists the default reply/forward quote style, then signals
    /// [`Surface::Settings`] so the host re-pulls.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly (this one just locks,
    // sets, and signals).
    #[allow(clippy::unused_async)]
    pub async fn set_default_quote_style(&self, style: QuoteStyleKind) {
        self.quote_settings
            .lock()
            .expect("quote-settings mutex poisoned")
            .set_style(to_style(style));
        self.observer.surface_changed(Surface::Settings);
    }

    /// Sets and persists whether the composer offers a per-message quote-style override, then
    /// signals [`Surface::Settings`] so the host re-pulls. With it off (the default) a reply or
    /// forward silently uses the app default and shows no picker.
    #[allow(clippy::unused_async)]
    pub async fn set_quote_style_per_message(&self, per_message: bool) {
        self.quote_settings
            .lock()
            .expect("quote-settings mutex poisoned")
            .set_per_message(per_message);
        self.observer.surface_changed(Surface::Settings);
    }
}

/// Maps the persisted style to the host-facing kind.
fn kind(style: QuoteStyle) -> QuoteStyleKind {
    match style {
        QuoteStyle::Indented => QuoteStyleKind::Indented,
        QuoteStyle::LineAndHeader => QuoteStyleKind::LineAndHeader,
    }
}

/// Maps the host-facing kind back to the persisted style.
fn to_style(kind: QuoteStyleKind) -> QuoteStyle {
    match kind {
        QuoteStyleKind::Indented => QuoteStyle::Indented,
        QuoteStyleKind::LineAndHeader => QuoteStyle::LineAndHeader,
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{MessageGrouping, QuoteStyle, load_preferences, save_preferences};
    use mailcal_viewmodel::QuoteStyleKind;

    use super::QuoteSettingsState;

    /// A preferences file with a sibling setting the quote-style writes must preserve.
    fn seeded(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        let seeded = mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();
        path
    }

    #[test]
    fn set_persists_and_reloads_without_clobbering_sibling_preferences() {
        let path = seeded("mailcal-quote-style-test");

        // The default loads (an indented quote), then a set persists the other style.
        let mut state = QuoteSettingsState::new(Some(path.clone()));
        assert_eq!(state.get().style, QuoteStyleKind::Indented);
        state.set_style(QuoteStyle::LineAndHeader);

        // A fresh state reads back the persisted choice, and the sibling prefs survived.
        assert_eq!(
            QuoteSettingsState::new(Some(path.clone())).get().style,
            QuoteStyleKind::LineAndHeader
        );
        let on_disk = load_preferences(&path);
        assert_eq!(on_disk.quote_style, QuoteStyle::LineAndHeader);
        assert_eq!(
            on_disk.display_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(on_disk.message_grouping, MessageGrouping::Flat);
    }

    #[test]
    fn the_per_message_override_is_off_by_default_and_persists_independently_of_the_style() {
        let path = seeded("mailcal-quote-per-message-test");

        // Off by default: the composer shows no picker until the user opts in.
        let mut state = QuoteSettingsState::new(Some(path.clone()));
        assert!(!state.get().per_message);

        // Turning it on persists, and leaves the chosen style alone.
        state.set_style(QuoteStyle::LineAndHeader);
        state.set_per_message(true);
        let reloaded = QuoteSettingsState::new(Some(path.clone()));
        assert!(reloaded.get().per_message);
        assert_eq!(reloaded.get().style, QuoteStyleKind::LineAndHeader);

        // And turning it back off leaves the style alone too: the two are independent.
        let mut state = QuoteSettingsState::new(Some(path.clone()));
        state.set_per_message(false);
        let on_disk = load_preferences(&path);
        assert!(!on_disk.quote_style_per_message);
        assert_eq!(on_disk.quote_style, QuoteStyle::LineAndHeader);
    }
}
