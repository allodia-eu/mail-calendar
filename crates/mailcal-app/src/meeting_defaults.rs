//! Settings → Videoconferencing past the accounts that make meetings: the person's Jitsi server
//! (`docs/conferencing.md`, rule 13) and each account's default video meeting (rule 10).

use engine_api::{JitsiMeetings, Provider};
use mailcal_viewmodel::calendar::{MeetingDefault, MeetingOption, VideoconferencingPage};

use super::video_meeting::JitsiServerRefused;
use crate::{App, Surface};

impl<P: Provider> App<P> {
    /// The page as it stands: the Jitsi server, the text last refused for it, and each account's
    /// default. Read from memory.
    #[must_use]
    pub fn videoconferencing_page(&self) -> VideoconferencingPage {
        let (jitsi_server, jitsi_refused) = {
            let prefs = self
                .calendar_prefs
                .lock()
                .expect("calendar-prefs mutex poisoned");
            (
                prefs.jitsi_server().map(str::to_owned),
                prefs.jitsi_refused().map(str::to_owned),
            )
        };
        VideoconferencingPage {
            jitsi_server,
            jitsi_refused,
            defaults: self.meeting_defaults(),
        }
    }

    /// The person's Jitsi server, or `None` until they set one (rule 13).
    #[must_use]
    pub fn jitsi_server(&self) -> Option<String> {
        self.calendar_prefs
            .lock()
            .expect("calendar-prefs mutex poisoned")
            .jitsi_server()
            .map(str::to_owned)
    }

    /// Stores `typed` as the person's Jitsi server in its stored form (trimmed, no trailing
    /// `/`), or forgets the server for `None` or blank text, and redraws the calendar rows whose
    /// choices it changes. Text that comes to what is already stored writes nothing, so a client
    /// sends the field each time it is left.
    ///
    /// A refusal is kept for the page (`jitsi_refused`) until the next text is sent, and raises
    /// [`Surface::Settings`].
    ///
    /// # Errors
    ///
    /// `JitsiServerRefused` for anything but an `https` address without a query or fragment,
    /// which then changes nothing stored.
    pub fn set_jitsi_server(&self, typed: Option<&str>) -> Result<(), JitsiServerRefused> {
        let server = typed
            .map(str::trim)
            .filter(|server| !server.is_empty())
            .map(|server| JitsiMeetings::new(server).map(|meetings| meetings.server().to_owned()))
            .transpose();
        let Ok(server) = server else {
            if self
                .calendar_prefs
                .lock()
                .expect("calendar-prefs mutex poisoned")
                .note_jitsi_refused(typed.map(str::to_owned))
            {
                self.observer.surface_changed(Surface::Settings);
            }
            return Err(JitsiServerRefused);
        };
        let (stored, refusal_cleared) = {
            let mut prefs = self
                .calendar_prefs
                .lock()
                .expect("calendar-prefs mutex poisoned");
            let refusal_cleared = prefs.note_jitsi_refused(None);
            let stored = prefs.jitsi_server() != server.as_deref();
            if stored {
                prefs.set_jitsi_server(server);
            }
            (stored, refusal_cleared)
        };
        if stored {
            self.calendar.resignal();
        } else if refusal_cleared {
            self.observer.surface_changed(Surface::Settings);
        }
        Ok(())
    }

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
