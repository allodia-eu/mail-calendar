//! The preferences half of the FFI intent surface, nested behind one `Intent::Preferences`
//! variant as the other families are.
//!
//! Each takes effect before `dispatch` returns, so the next read, and the next intent, see it;
//! the preferences file is written behind, and a write that fails is a notice
//! (`NoticeKind::SettingsNotSaved`), never a value the client keeps.

use crate::{
    Appearance, CalendarLayout, QuoteStyleKind, SwipeActionKind, SwipeDirection, TimeFormat,
    WeekStart,
};

/// A change to one of the person's preferences, from Settings or from the control that also
/// changes it elsewhere (the reading pane's dark-background toggle, a pinch on the calendar).
#[derive(uniffi::Enum)]
pub enum PreferenceIntent {
    /// Whether the app paints light, dark, or however the host is set. Signals
    /// `Surface::Settings` only: nothing the core computes depends on it.
    SetAppearance { appearance: Appearance },
    /// The 12/24-hour clock, for mail and calendar alike. Signals `Surface::Settings` and
    /// `Surface::Calendar`.
    SetTimeFormat { format: TimeFormat },
    /// The first day of the calendar week. Signals `Surface::Settings` and `Surface::Calendar`.
    SetWeekStart { start: WeekStart },
    /// Whether a message is drawn on a dark background while the app is dark. A client draws
    /// the open message again on the `Surface::Settings` signal that follows.
    SetDarkMessageBackground { dark: bool },
    /// How many hours the calendar grid shows at once, clamped by the core: a pinch runs off the
    /// end of its gesture. Settings offers `DisplaySettings::visible_hours_choices`.
    SetCalendarVisibleHours { hours: u8 },
    /// The shape the calendar is being read in, so it opens that way next time.
    SetCalendarLayout { layout: CalendarLayout },
    /// The account a new message composes from in the unified view; `None` restores the first
    /// configured account.
    SetDefaultSendAccount { account: Option<String> },
    /// What a swipe across a message row in `direction` does.
    SetSwipeAction {
        direction: SwipeDirection,
        action: SwipeActionKind,
    },
    /// The default reply and forward quoting.
    SetQuoteStyle { style: QuoteStyleKind },
    /// Whether the composer offers a quoting choice of its own for each message.
    SetQuoteStylePerMessage { per_message: bool },
    /// The calendar new events are filed on. `None` for either half returns to "the first
    /// writable calendar", which is not the same as naming one.
    SetDefaultCalendar {
        account: Option<String>,
        calendar: Option<String>,
    },
    /// Shows or hides one calendar's events.
    SetCalendarVisible {
        account: String,
        calendar: String,
        visible: bool,
    },
    /// One calendar's colour, as `#rrggbb` from `calendar_palette`, or the server's for `None`.
    SetCalendarColor {
        account: String,
        calendar: String,
        hex: Option<String>,
    },
    /// Opens or shuts `account`'s group of calendars in the calendar pane; persisted, and
    /// signals `Surface::Calendar`.
    SetCalendarGroupExpanded { account: String, expanded: bool },
    /// `account`'s default video meeting, one of its `MeetingDefault::choices` ids, or none.
    SetDefaultMeeting {
        account: String,
        choice: Option<String>,
    },
    /// The analytics decision. Send `true` only from a deliberate, affirmative action (a switch
    /// the person moved, a button they pressed), never from a pre-checked box. Opting in mints
    /// the install id; opting out clears it and asks the backend to erase what it holds under
    /// it. Repeating the decision in force changes nothing.
    SetAnalyticsConsent { enabled: bool },
}
