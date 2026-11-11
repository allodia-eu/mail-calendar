//! Each account's default video meeting (`docs/conferencing.md`, rule 10): what Settings →
//! Videoconferencing lists, and the choice it stores.

use engine_api::Provider;
use mailcal_viewmodel::calendar::{MeetingDefault, MeetingOption};

use crate::App;

impl<P: Provider> App<P> {
    /// Every account whose calendars offer a video meeting, in the calendars' order, each with
    /// the meetings they offer and the one its new events take.
    #[must_use]
    pub fn meeting_defaults(&self) -> Vec<MeetingDefault> {
        let mut defaults: Vec<MeetingDefault> = Vec::new();
        for row in self.calendars() {
            if row.meeting_choices.is_empty() {
                continue;
            }
            let index = defaults
                .iter()
                .position(|default| default.account == row.account)
                .unwrap_or_else(|| {
                    defaults.push(MeetingDefault {
                        account: row.account.clone(),
                        account_address: row.account_address.clone(),
                        choices: Vec::new(),
                        chosen: None,
                    });
                    defaults.len() - 1
                });
            let default = &mut defaults[index];
            let new: Vec<MeetingOption> = row
                .meeting_choices
                .into_iter()
                .filter(|choice| !default.choices.contains(choice))
                .collect();
            default.choices.extend(new);
            default.chosen = default.chosen.take().or(row.default_meeting);
        }
        defaults
    }

    /// Stores the video meeting `account`'s new events take once they have an invitee, as one of
    /// its calendars' `meeting_choices` ids, or none for `None`, and redraws the calendar rows.
    /// A choice no calendar of the account offers is stored and stays without effect until one
    /// does.
    pub fn set_default_meeting(&self, account: &str, choice: Option<&str>) {
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .set_default_meeting(account, choice.map(str::to_owned));
        self.calendar.resignal();
    }
}
