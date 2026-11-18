//! The display settings on [`MailcalApp`]: first day of the week, 12/24-hour clock, light/dark
//! appearance, and the calendar's default horizon.
//!
//! All of them are persisted **in the core**, not in each client; three clients disagreeing about
//! which day a week starts on is not a cosmetic bug, it silently shifts every column of the grid.
//! They are settings rather than locale defaults on purpose: a locale default is invisible and
//! unoverridable, so a user on an `en-US` phone would get a Sunday-start week and a 12-hour clock
//! with no way to say otherwise. The defaults are European; Monday, 24-hour.
//!
//! A change signals both `Surface::Settings` and `Surface::Calendar`, so a host re-pulls the
//! settings screen *and* the grid it is showing. The appearance is the exception: it signals
//! Settings alone, because the core computes nothing from it.

use mailcal_account::{
    Appearance as AppAppearance, CalendarLayout as AppCalendarLayout, TimeFormat as AppTimeFormat,
    WeekStart as AppWeekStart, load_preferences, preferences_path, visible_hours_choices,
};
use mailcal_app::DisplaySettings as AppDisplaySettings;

use crate::{Appearance, CalendarLayout, DisplaySettings, MailcalApp, TimeFormat, WeekStart};

/// The persisted [`Appearance`] read straight out of `data_dir`, with no app and no engine.
///
/// A client needs it **before** [`MailcalApp`] exists. Building one opens the engine store and
/// starts dialing every account, and a window painted in the host's scheme until that returns is a
/// visible flash of exactly the theme the user said they did not want. This reads one small TOML
/// file, so it is cheap enough to sit in front of the first frame.
///
/// Names the same file the app writes ([`preferences_path`]), and falls back to the product default
/// (follow the host) for a missing, unreadable or unparseable one.
#[uniffi::export]
#[must_use]
pub fn stored_appearance(data_dir: String) -> Appearance {
    load_preferences(preferences_path(data_dir))
        .appearance
        .into()
}

#[uniffi::export]
impl MailcalApp {
    /// The display settings. Pull after a `Surface::Settings` signal.
    pub fn display_settings(&self) -> DisplaySettings {
        self.app.display_settings().into()
    }
}

impl From<AppDisplaySettings> for DisplaySettings {
    fn from(settings: AppDisplaySettings) -> Self {
        Self {
            week_start: settings.week_start.into(),
            time_format: settings.time_format.into(),
            appearance: settings.appearance.into(),
            dark_message_background: settings.dark_message_background,
            visible_hours: settings.visible_hours,
            visible_hours_choices: visible_hours_choices(settings.visible_hours),
            layout: settings.layout.into(),
        }
    }
}

impl From<AppCalendarLayout> for CalendarLayout {
    fn from(layout: AppCalendarLayout) -> Self {
        match layout {
            AppCalendarLayout::Day => Self::Day,
            AppCalendarLayout::ThreeDay => Self::ThreeDay,
            AppCalendarLayout::WorkWeek => Self::WorkWeek,
            AppCalendarLayout::Week => Self::Week,
            AppCalendarLayout::Month => Self::Month,
            AppCalendarLayout::Agenda => Self::Agenda,
        }
    }
}

impl From<CalendarLayout> for AppCalendarLayout {
    fn from(layout: CalendarLayout) -> Self {
        match layout {
            CalendarLayout::Day => Self::Day,
            CalendarLayout::ThreeDay => Self::ThreeDay,
            CalendarLayout::WorkWeek => Self::WorkWeek,
            CalendarLayout::Week => Self::Week,
            CalendarLayout::Month => Self::Month,
            CalendarLayout::Agenda => Self::Agenda,
        }
    }
}

impl From<AppWeekStart> for WeekStart {
    fn from(start: AppWeekStart) -> Self {
        match start {
            AppWeekStart::Monday => Self::Monday,
            AppWeekStart::Sunday => Self::Sunday,
        }
    }
}

impl From<WeekStart> for AppWeekStart {
    fn from(start: WeekStart) -> Self {
        match start {
            WeekStart::Monday => Self::Monday,
            WeekStart::Sunday => Self::Sunday,
        }
    }
}

impl From<AppTimeFormat> for TimeFormat {
    fn from(format: AppTimeFormat) -> Self {
        match format {
            AppTimeFormat::TwentyFourHour => Self::TwentyFourHour,
            AppTimeFormat::TwelveHour => Self::TwelveHour,
        }
    }
}

impl From<TimeFormat> for AppTimeFormat {
    fn from(format: TimeFormat) -> Self {
        match format {
            TimeFormat::TwentyFourHour => Self::TwentyFourHour,
            TimeFormat::TwelveHour => Self::TwelveHour,
        }
    }
}

impl From<AppAppearance> for Appearance {
    fn from(appearance: AppAppearance) -> Self {
        match appearance {
            AppAppearance::System => Self::System,
            AppAppearance::Light => Self::Light,
            AppAppearance::Dark => Self::Dark,
        }
    }
}

impl From<Appearance> for AppAppearance {
    fn from(appearance: Appearance) -> Self {
        match appearance {
            Appearance::System => Self::System,
            Appearance::Light => Self::Light,
            Appearance::Dark => Self::Dark,
        }
    }
}
