//! The display preferences the whole app reads: the first day of the week, the 12/24-hour clock,
//! whether the app paints light or dark, and the calendar's default horizon.
//!
//! These live in the core rather than in each client for the same reason the quote style and the
//! swipe actions do (three clients cannot be allowed to disagree) and they are *settings* rather
//! than locale defaults because a locale default is invisible and unoverridable. A user on an
//! `en-US` phone would silently get a Sunday-start week and a 12-hour clock with no way to say
//! otherwise.
//!
//! Mirrors [`crate::quote_settings`]: this state holds the loaded values and writes each back
//! read-modify-write, so the sibling preferences in the same file are preserved.

use std::path::PathBuf;

use engine_api::Provider;
use mailcal_account::{
    Appearance, CalendarLayout, Preferences, TimeFormat, WeekStart, clamp_visible_hours,
    load_preferences, save_preferences,
};

use crate::{App, Surface};

/// The display preferences, as the host reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplaySettings {
    /// Which day a calendar week begins on. Monday by default.
    pub week_start: WeekStart,
    /// Whether times render on a 24-hour clock, in **mail and calendar alike**. 24-hour by
    /// default.
    pub time_format: TimeFormat,
    /// Whether the app paints itself light, dark, or however the host is set. Follows the host by
    /// default.
    pub appearance: Appearance,
    /// How many hours of the day the calendar grid shows at once: the horizon a pinch zooms.
    /// Always within the core's clamp, so a host can divide by it without checking.
    pub visible_hours: u8,
    /// The shape the calendar opens in: the last one the user chose.
    pub layout: CalendarLayout,
}

/// The loaded display preferences and where to persist them.
pub(crate) struct DisplaySettingsState {
    settings: DisplaySettings,
    prefs_path: Option<PathBuf>,
}

impl DisplaySettingsState {
    /// Loads the persisted choices (the product defaults when absent or unreadable).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map_or_else(Preferences::default, load_preferences);
        Self {
            settings: DisplaySettings {
                week_start: prefs.week_start,
                time_format: prefs.time_format,
                appearance: prefs.appearance,
                visible_hours: prefs.visible_hours(),
                layout: prefs.calendar_layout,
            },
            prefs_path,
        }
    }

    pub(crate) fn get(&self) -> DisplaySettings {
        self.settings
    }

    /// Applies `edit` to the in-memory settings and persists the result, preserving every sibling
    /// preference in the same file.
    fn update(&mut self, edit: impl FnOnce(&mut DisplaySettings)) {
        edit(&mut self.settings);
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.week_start = self.settings.week_start;
            prefs.time_format = self.settings.time_format;
            prefs.appearance = self.settings.appearance;
            prefs.calendar_visible_hours = self.settings.visible_hours;
            prefs.calendar_layout = self.settings.layout;
            let _ = save_preferences(path, &prefs);
        }
    }
}

impl<P: Provider> App<P> {
    /// The display preferences (pulled after a [`Surface::Settings`] signal).
    #[must_use]
    pub fn display_settings(&self) -> DisplaySettings {
        self.display_settings
            .lock()
            .expect("display-settings mutex poisoned")
            .get()
    }

    /// Sets and persists the first day of the week.
    ///
    /// Signals [`Surface::Settings`] **and** [`Surface::Calendar`]: the grid's columns are laid out
    /// around this, so every page a client is showing is now stale and must be re-pulled.
    /// Signalling only Settings would leave the user staring at a week that still starts on the
    /// old day.
    // `async` with no inner `await` is intentional: every dispatched command method shares one
    // async shape so `dispatch` and the FFI adapter drive them uniformly.
    #[allow(clippy::unused_async)]
    pub async fn set_week_start(&self, start: WeekStart) {
        self.edit_display(|settings| settings.week_start = start);
    }

    /// Sets and persists the 12/24-hour clock, for mail and calendar alike.
    #[allow(clippy::unused_async)]
    pub async fn set_time_format(&self, format: TimeFormat) {
        self.edit_display(|settings| settings.time_format = format);
    }

    /// Sets and persists whether the app paints light, dark, or however the host is set.
    ///
    /// Signals only [`Surface::Settings`], unlike its neighbours: nothing the core computes depends
    /// on it. Every swatch already carries both a light and a dark form, and the client picks one;
    /// so the grid a host is showing is still correct, and re-pulling it would buy nothing.
    // `async` with no inner `await` is intentional; see `set_week_start`.
    #[allow(clippy::unused_async)]
    pub async fn set_appearance(&self, appearance: Appearance) {
        self.display_settings
            .lock()
            .expect("display-settings mutex poisoned")
            .update(|settings| settings.appearance = appearance);
        self.observer.surface_changed(Surface::Settings);
    }

    /// Sets and persists the calendar's default horizon; how many hours the grid shows at once.
    ///
    /// **Clamped here**, not in the client: a pinch runs off the end of its own gesture all the
    /// time, and a client that sent the raw value would leave one platform showing a 1-hour day.
    #[allow(clippy::unused_async)]
    pub async fn set_calendar_visible_hours(&self, hours: u8) {
        let hours = clamp_visible_hours(hours);
        self.edit_display(|settings| settings.visible_hours = hours);
    }

    /// Remembers the shape the calendar is being read in, so it opens that way next time.
    ///
    /// The other half of the horizon: between them, a pinch to a single day is fully restored on
    /// the next launch: the same columns, over the same hours.
    #[allow(clippy::unused_async)]
    pub async fn set_calendar_layout(&self, layout: CalendarLayout) {
        self.edit_display(|settings| settings.layout = layout);
    }

    /// Applies an edit, persists it, and tells the host that both settings and the grid are stale.
    fn edit_display(&self, edit: impl FnOnce(&mut DisplaySettings)) {
        self.display_settings
            .lock()
            .expect("display-settings mutex poisoned")
            .update(edit);
        self.observer.surface_changed(Surface::Settings);
        // The grid pull recomputes from the settings just written, so there is no new snapshot
        // to publish: only the fact that the old one is stale.
        self.calendar.resignal();
    }
}

#[cfg(test)]
mod tests {
    use mailcal_account::{
        Appearance, CalendarLayout, DEFAULT_VISIBLE_HOURS, MAX_VISIBLE_HOURS, MIN_VISIBLE_HOURS,
        MessageGrouping, QuoteStyle, TimeFormat, WeekStart, clamp_visible_hours, load_preferences,
        save_preferences,
    };

    use super::DisplaySettingsState;

    #[test]
    fn the_product_defaults_are_european_and_do_not_depend_on_the_device() {
        // Europe is where this product's users are. A user who never opens settings must get a
        // Monday-start week and a 24-hour clock: not whatever their phone's locale implies.
        let settings = DisplaySettingsState::new(None).get();
        assert_eq!(settings.week_start, WeekStart::Monday);
        assert_eq!(settings.time_format, TimeFormat::TwentyFourHour);
        assert_eq!(settings.visible_hours, DEFAULT_VISIBLE_HOURS);
        assert!(settings.week_start.starts_monday());
        assert!(settings.time_format.is_24_hour());
        // The one default that deliberately DOES follow the device: a light/dark setting is
        // visible and overridable on every host, so adopting it is a real default, not a hidden
        // one. It is asserted here so a future "sensible" flip to Light has to argue with a test.
        assert_eq!(settings.appearance, Appearance::System);
    }

    #[test]
    fn a_horizon_is_clamped_rather_than_trusted() {
        // A pinch runs off the end of its own gesture constantly, and the preferences file is TOML
        // a user can hand-edit. Zero visible hours would divide the grid by nothing.
        assert_eq!(clamp_visible_hours(0), MIN_VISIBLE_HOURS);
        assert_eq!(clamp_visible_hours(1), MIN_VISIBLE_HOURS);
        assert_eq!(clamp_visible_hours(200), MAX_VISIBLE_HOURS);
        assert_eq!(clamp_visible_hours(8), 8);

        // And a preferences file carrying a junk value still yields a usable grid.
        let dir = std::env::temp_dir().join("mailcal-display-clamp-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        let seeded = mailcal_account::Preferences {
            calendar_visible_hours: 0,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();
        assert_eq!(
            DisplaySettingsState::new(Some(path)).get().visible_hours,
            MIN_VISIBLE_HOURS
        );
    }

    #[test]
    fn each_setting_persists_without_clobbering_its_siblings() {
        let dir = std::env::temp_dir().join("mailcal-display-settings-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("preferences.toml");
        // Seed unrelated preferences the display writes must preserve.
        let seeded = mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            quote_style: QuoteStyle::LineAndHeader,
            ..Default::default()
        };
        save_preferences(&path, &seeded).unwrap();

        // A file written before these settings existed carries none of the fields; it must still
        // read back as the product defaults, not as serde's zero/false.
        let mut state = DisplaySettingsState::new(Some(path.clone()));
        assert_eq!(state.get().week_start, WeekStart::Monday);
        assert_eq!(state.get().time_format, TimeFormat::TwentyFourHour);
        assert_eq!(state.get().appearance, Appearance::System);
        assert_eq!(state.get().visible_hours, DEFAULT_VISIBLE_HOURS);
        // The whole week, and not a subset of it: a narrower default leaves the rest of the page
        // hanging off the screen as a scrollable overhang, which on a phone swallows the swipe.
        assert_eq!(state.get().layout, CalendarLayout::Week);

        state.update(|settings| settings.week_start = WeekStart::Sunday);
        state.update(|settings| settings.time_format = TimeFormat::TwelveHour);
        state.update(|settings| settings.appearance = Appearance::Dark);
        state.update(|settings| settings.visible_hours = 6);
        state.update(|settings| settings.layout = CalendarLayout::ThreeDay);

        // A fresh state reads back all three, and the siblings survived every write.
        let reloaded = DisplaySettingsState::new(Some(path.clone())).get();
        assert_eq!(reloaded.week_start, WeekStart::Sunday);
        assert_eq!(reloaded.time_format, TimeFormat::TwelveHour);
        assert_eq!(reloaded.appearance, Appearance::Dark);
        assert_eq!(reloaded.visible_hours, 6);
        // The shape the user was last reading in, restored. Together with the horizon above, a
        // pinch down to three days over eight hours comes back exactly as it was left.
        assert_eq!(reloaded.layout, CalendarLayout::ThreeDay);

        let on_disk = load_preferences(&path);
        assert_eq!(
            on_disk.display_timezone.as_deref(),
            Some("Europe/Amsterdam")
        );
        assert_eq!(on_disk.message_grouping, MessageGrouping::Flat);
        assert_eq!(on_disk.quote_style, QuoteStyle::LineAndHeader);
    }
}
