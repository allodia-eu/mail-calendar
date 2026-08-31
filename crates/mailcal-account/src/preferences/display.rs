//! The **display** preferences: which day a week starts on, the 12/24-hour clock, whether the app
//! paints light or dark, how much of the day the calendar shows at once, and what the user decided
//! about each individual calendar.
//!
//! Split out of the parent module, which was over the 500-line limit. These are the preferences a
//! user changes to make the app *look* the way they want; its sibling `behavior` holds the ones
//! about how it *reads and composes*, and the parent the rest.
//!
//! The clock and calendar defaults here are European by design (Monday, 24-hour) and
//! deliberately **not** derived from the device locale: a locale default is invisible and
//! unoverridable. [`Appearance`] is the exception that proves it: the host's light/dark setting is
//! visible and overridable, so following it is a real default rather than a hidden one.

use serde::{Deserialize, Serialize};

/// Which day a calendar week begins on, as a persisted app-level preference.
///
/// **Defaults to Monday**: the ISO-8601 week, and the convention across Europe, where this
/// product's users are. It is deliberately *not* derived from the device locale: a locale default
/// is invisible and unoverridable, so a user whose phone is set to `en-US` would silently get a
/// Sunday-start week with no way to say otherwise.
///
/// This is not cosmetic. Get it wrong and every column of the grid shifts, so the user reads
/// Tuesday's meetings under Monday's heading.
///
/// Saturday-start weeks (much of the Middle East) are a real convention this does not yet cover;
/// see `docs/calendar.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeekStart {
    /// The week begins on Monday; ISO-8601, and the European convention. The default.
    #[default]
    Monday,
    /// The week begins on Sunday: the convention in the US and much of Asia.
    Sunday,
}

impl WeekStart {
    /// Whether the week begins on Monday: the form the grid's day-axis maths takes.
    #[must_use]
    pub fn starts_monday(self) -> bool {
        matches!(self, Self::Monday)
    }
}

/// Whether times are shown on a 24-hour or a 12-hour clock, as a persisted app-level preference.
///
/// **Defaults to 24-hour**: the European convention. It spans **mail and calendar alike**: a
/// message list that reads `14:05` beside a calendar that reads `2 PM` is one app disagreeing with
/// itself, which is exactly what happened before this setting existed (the mail list hard-coded
/// 24-hour while the calendar read the device's own clock setting).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeFormat {
    /// `14:05`. The default.
    #[default]
    TwentyFourHour,
    /// `2:05 PM`.
    TwelveHour,
}

impl TimeFormat {
    /// Whether times render on a 24-hour clock.
    #[must_use]
    pub fn is_24_hour(self) -> bool {
        matches!(self, Self::TwentyFourHour)
    }
}

/// Whether the app paints itself light or dark, as a persisted app-level preference.
///
/// **Defaults to [`Appearance::System`]**: the host's own light/dark setting, followed while the
/// app runs, which is what every client did before this preference existed. The other two are an
/// explicit override for a user who wants their mail in the scheme their desktop is not set to.
///
/// Unlike [`WeekStart`] and [`TimeFormat`] this changes nothing the core computes: the calendar
/// already emits both halves of every swatch and the client picks one. It lives here so the choice
/// survives a restart and so four clients cannot each invent their own default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Follow the host's light/dark setting, including while the app is running. The default.
    #[default]
    System,
    /// Light, whatever the host is set to.
    Light,
    /// Dark, whatever the host is set to.
    Dark,
}

/// Which shape the calendar opens in: the last one the user was looking at.
///
/// A persisted **core** setting rather than client state, for the same reason as [`WeekStart`]: it
/// is the user's choice, and it must survive the app being closed. It also has to survive being
/// *read* by more than one client, and a client-local default is how the macOS app and the phone
/// end up disagreeing about what "the calendar" looks like.
///
/// **Defaults to the whole week.** Not an arbitrary preference: the four grid shapes are zoom
/// levels of one grid, and the page underneath is always a whole week: so any shape narrower than
/// a week leaves the rest of it hanging off the side of the screen as a scrollable overhang. On a
/// phone that overhang sits *inside* the pager and takes the swipe before the pager does, so a
/// flick meant for next week is spent sliding along this one. Opening on the week means the columns
/// fill the screen exactly, and every swipe turns the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarLayout {
    /// One day at a time.
    Day,
    /// Three days.
    ThreeDay,
    /// Monday to Friday.
    WorkWeek,
    /// The whole week. The default.
    #[default]
    Week,
    /// The month grid: a different layout, not a zoom level.
    Month,
    /// The agenda list; likewise.
    Agenda,
}

/// The narrowest calendar horizon the grid will zoom to; below this an event's label cannot fit
/// inside its own block, and the grid becomes a wall of unreadable colour.
pub const MIN_VISIBLE_HOURS: u8 = 4;

/// The widest horizon: the whole day at once.
pub const MAX_VISIBLE_HOURS: u8 = 24;

/// The default horizon: how many hours of the day the calendar grid shows at once before the user
/// pinches. A working day's worth, which is what a calendar is usually opened to look at.
pub const DEFAULT_VISIBLE_HOURS: u8 = 12;

/// Clamps a requested calendar horizon into [`MIN_VISIBLE_HOURS`]..=[`MAX_VISIBLE_HOURS`].
///
/// Validation lives here rather than in each client so a pinch that runs off the end of its gesture
/// cannot leave one client showing a 1-hour day and another a 40-hour one.
#[must_use]
pub fn clamp_visible_hours(hours: u8) -> u8 {
    hours.clamp(MIN_VISIBLE_HOURS, MAX_VISIBLE_HOURS)
}

/// The serde default for [`Preferences::calendar_visible_hours`]: the field is a `u8`, whose own
/// `Default` is **zero**, and a zero-hour day would divide the grid by nothing.
pub(super) fn default_visible_hours() -> u8 {
    DEFAULT_VISIBLE_HOURS
}

/// The serde default for [`CalendarPrefs::visible`]: a calendar nobody has touched is **shown**.
/// `bool`'s own `Default` is `false`, which would hide every calendar the user has never opened the
/// manager for, i.e. all of them.
fn visible_by_default() -> bool {
    true
}

/// What the user has decided about one calendar: whether to draw it, and what colour to draw it in.
///
/// Absent means "untouched", which is **visible, in the server's colour**: not hidden and not
/// grey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarPrefs {
    /// Whether this calendar's events are drawn. Defaults to shown.
    #[serde(default = "visible_by_default")]
    pub visible: bool,
    /// The user's colour override (`#rrggbb`), or `None` to keep the colour the server sent (which
    /// the core snaps to the nearest palette entry).
    #[serde(default)]
    pub color: Option<String>,
}

impl Default for CalendarPrefs {
    fn default() -> Self {
        Self {
            visible: true,
            color: None,
        }
    }
}

/// Which calendar a new event is filed on unless the user picks another in the editor.
///
/// Stored as the pair that names a calendar, because a calendar id is unique only *within* its
/// account: two accounts can each have a `work`, and the id alone would name both.
///
/// Absent until someone chooses, and a choice can go stale, because the calendar it names may be
/// removed with its account, or turn read-only when a share is downgraded. So this is the *stored*
/// choice, never the effective one; resolving it against the calendars that actually exist is
/// [`crate::Preferences`]'s caller's job, and it happens in exactly one place so four clients
/// cannot disagree about which calendar "the default" is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultCalendar {
    /// The owning account's id.
    pub account: String,
    /// The calendar's provider key, unique within that account.
    pub calendar: String,
}
