//! What an `Intent::Preferences` does, and the notice a save that failed raises.
//!
//! Applied on the host's thread, before `dispatch` returns: each change only updates what the core
//! holds and hands the preferences file a change to save behind, so it waits on nothing, and two
//! changes sent in order take effect in that order. A save that fails is
//! `NoticeKind::SettingsNotSaved`, whose Try again writes what is waiting.

use mailcal_account::DefaultCalendar;

use crate::{MailcalApp, PreferenceIntent};

impl MailcalApp {
    /// Handles an `Intent::Preferences`.
    pub(crate) fn dispatch_preference(&self, intent: PreferenceIntent) {
        let app = &self.app;
        match intent {
            PreferenceIntent::SetAppearance { appearance } => {
                self.runtime
                    .block_on(|| app.set_appearance(appearance.into()));
            }
            PreferenceIntent::SetTimeFormat { format } => {
                self.runtime.block_on(|| app.set_time_format(format.into()));
            }
            PreferenceIntent::SetWeekStart { start } => {
                self.runtime.block_on(|| app.set_week_start(start.into()));
            }
            PreferenceIntent::SetDarkMessageBackground { dark } => {
                self.runtime
                    .block_on(|| app.set_dark_message_background(dark));
            }
            PreferenceIntent::SetCalendarVisibleHours { hours } => {
                self.runtime
                    .block_on(|| app.set_calendar_visible_hours(hours));
            }
            PreferenceIntent::SetCalendarLayout { layout } => {
                self.runtime
                    .block_on(|| app.set_calendar_layout(layout.into()));
            }
            PreferenceIntent::SetDefaultSendAccount { account } => {
                self.runtime
                    .block_on(|| app.set_default_send_account(account));
            }
            PreferenceIntent::SetSwipeAction { direction, action } => {
                self.runtime
                    .block_on(|| app.set_swipe_action(direction.into(), action.into()));
            }
            PreferenceIntent::SetQuoteStyle { style } => {
                self.runtime
                    .block_on(|| app.set_default_quote_style(style.into()));
            }
            PreferenceIntent::SetQuoteStylePerMessage { per_message } => {
                self.runtime
                    .block_on(|| app.set_quote_style_per_message(per_message));
            }
            PreferenceIntent::SetDefaultCalendar { account, calendar } => {
                let choice = account
                    .zip(calendar)
                    .map(|(account, calendar)| DefaultCalendar { account, calendar });
                self.runtime.block_on(|| app.set_default_calendar(choice));
            }
            PreferenceIntent::SetCalendarVisible {
                account,
                calendar,
                visible,
            } => {
                self.runtime
                    .block_on(|| app.set_calendar_visible(&account, &calendar, visible));
            }
            PreferenceIntent::SetCalendarColor {
                account,
                calendar,
                hex,
            } => {
                self.runtime
                    .block_on(|| app.set_calendar_color(&account, &calendar, hex));
            }
            PreferenceIntent::SetDefaultMeeting { account, choice } => {
                app.set_default_meeting(&account, choice.as_deref());
            }
            PreferenceIntent::SetAnalyticsConsent { enabled } => {
                app.set_analytics_consent(enabled);
            }
        }
    }

    /// The Try again of `NoticeKind::SettingsNotSaved`: writes what is waiting, off the host's
    /// thread, and settles the notice's retry once the attempt has ended.
    pub(crate) fn retry_preferences_save(&self) {
        let Some(this) = self.rediscovery.app() else {
            return;
        };
        std::thread::spawn(move || {
            this.app.retry_preferences_save();
            this.settle_notice_retries();
        });
    }
}

#[cfg(test)]
#[path = "app_preferences_tests.rs"]
mod tests;
