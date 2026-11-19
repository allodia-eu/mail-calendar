//! The FFI half of the editor's video meeting (`docs/conferencing.md`, rules 9 to 15): what a
//! calendar offers, what the editor opens showing, what a write asks for, and Settings →
//! Videoconferencing's page.

use engine_api::ConferenceText;
use mailcal_account::{
    EditorMeeting as AppEditorMeeting, MeetingChange as AppMeetingChange, MeetingChoice,
};
use mailcal_viewmodel::calendar::{
    MeetingDefault as AppMeetingDefault, MeetingOption as AppMeetingOption,
    VideoconferencingPage as AppVideoconferencingPage,
};

use crate::MailcalApp;

/// One video meeting a calendar's editor offers.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingOption {
    /// What a write sends back to choose it (`MeetingChange::Set`'s `choice`).
    pub id: String,
    /// The service's name, a brand name the same in every language.
    pub name: String,
    /// The address of the account that makes it, for a meeting made through an account the
    /// person connected, which a client shows beside the name (`event_meeting_through`); `None`
    /// for the calendar's own server's and for Jitsi.
    #[uniffi(default = None)]
    pub through: Option<String>,
}

impl From<AppMeetingOption> for MeetingOption {
    fn from(option: AppMeetingOption) -> Self {
        Self {
            id: option.id,
            name: option.name,
            through: option.through,
        }
    }
}

/// One account's default video meeting, as Settings → Videoconferencing lists it
/// (`docs/conferencing.md`, rule 10).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingDefault {
    /// The account whose calendars the default is for.
    pub account: String,
    /// What a person reads the account by.
    pub account_address: String,
    /// Every meeting a calendar of the account offers, in order; a client lists None first.
    pub choices: Vec<MeetingOption>,
    /// The `MeetingOption::id` of the chosen one, or `None` for none.
    pub chosen: Option<String>,
}

impl From<AppMeetingDefault> for MeetingDefault {
    fn from(default: AppMeetingDefault) -> Self {
        Self {
            account: default.account,
            account_address: default.account_address,
            choices: default.choices.into_iter().map(Into::into).collect(),
            chosen: default.chosen,
        }
    }
}

/// Settings → Videoconferencing past the accounts that make meetings (`AccountsSnapshot::
/// meeting_accounts`): the person's Jitsi server and each account's default video meeting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct VideoconferencingPage {
    /// The Jitsi server as stored, which the field shows, or `None` until the person sets one.
    pub jitsi_server: Option<String>,
    /// The text last refused as the Jitsi server, exactly as it was sent, until the next is sent.
    /// The field says the address was refused while it still holds this text.
    pub jitsi_refused: Option<String>,
    /// Every account whose calendars offer a video meeting, each with what they offer and the
    /// one its new events take once they have an invitee; drawn only while there is one.
    pub defaults: Vec<MeetingDefault>,
}

impl From<AppVideoconferencingPage> for VideoconferencingPage {
    fn from(page: AppVideoconferencingPage) -> Self {
        Self {
            jitsi_server: page.jitsi_server,
            jitsi_refused: page.jitsi_refused,
            defaults: page.defaults.into_iter().map(Into::into).collect(),
        }
    }
}

/// An account that can make video meetings, as Settings → Videoconferencing lists it: which
/// service, through which account, and whether the account is used for them. Changing that is
/// the account page's switch, which the row opens.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingAccount {
    /// The account, as a link names one.
    pub account: crate::LinkedAccount,
    /// The service's name, a brand name the same in every language.
    pub service: String,
    /// The account's Video meetings use, as its page shows it.
    pub state: crate::CapabilityState,
}

/// The meeting an editor opens showing: the event's first shown one.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EditorMeeting {
    /// The service's name, or `None` when nothing names it; the client then shows its own words
    /// for a video meeting.
    pub name: Option<String>,
    /// The `MeetingOption::id` of the choice that made it, or `None` for a meeting no choice
    /// makes (a Zoom link from an invitation): the editor shows it by name, and offers to take it
    /// off but not to choose it again.
    pub choice: Option<String>,
}

impl From<AppEditorMeeting> for EditorMeeting {
    fn from(meeting: AppEditorMeeting) -> Self {
        Self {
            name: meeting.name,
            choice: meeting.choice.as_ref().map(MeetingChoice::id),
        }
    }
}

/// The words of the block a meeting made elsewhere leaves in the notes, from the client's
/// catalog in the app's language: the core has no locale.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeetingWords {
    /// The heading for a meeting without a name of its own ("Video meeting").
    pub meeting: String,
    /// Before the join link ("Join").
    pub join: String,
    /// Before the meeting ID ("Meeting ID").
    pub meeting_id: String,
    /// Before the passcode ("Passcode").
    pub passcode: String,
    /// Before each dial-in number ("Dial in").
    pub dial_in: String,
    /// Before the code a caller enters after dialling ("Conference ID").
    pub dial_in_code: String,
}

impl From<MeetingWords> for ConferenceText {
    fn from(words: MeetingWords) -> Self {
        Self {
            meeting: words.meeting,
            join: words.join,
            meeting_id: words.meeting_id,
            passcode: words.passcode,
            dial_in: words.dial_in,
            dial_in_code: words.dial_in_code,
        }
    }
}

/// What a write does to an event's video meeting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeetingChange {
    /// Take the event's meeting off, with the block and the location it left.
    Remove,
    /// Put the meeting the calendar offers as `choice` on the event, replacing the one it has.
    Set {
        /// A `MeetingOption::id` of the event's calendar.
        choice: String,
        /// The words of the block its notes carry when the meeting is made elsewhere.
        words: MeetingWords,
    },
}

/// The core's change for `change`; refused for a choice no calendar offers.
pub(crate) fn meeting_change(
    change: Option<MeetingChange>,
) -> Result<Option<AppMeetingChange>, String> {
    Ok(match change {
        None => None,
        Some(MeetingChange::Remove) => Some(AppMeetingChange::Remove),
        Some(MeetingChange::Set { choice, words }) => Some(AppMeetingChange::Set {
            choice: MeetingChoice::from_id(&choice)
                .ok_or_else(|| "no calendar offers that video meeting".to_owned())?,
            words: Box::new(words.into()),
        }),
    })
}

#[uniffi::export]
impl MailcalApp {
    /// Settings → Videoconferencing's page, read from memory. A client reads it again on
    /// `Surface::Calendar`, which every change to what the calendars offer raises, and on
    /// `Surface::Settings`, which a refused Jitsi server raises.
    pub fn videoconferencing_page(&self) -> VideoconferencingPage {
        self.app.videoconferencing_page().into()
    }
}

#[cfg(test)]
#[path = "records_video_meeting_tests.rs"]
mod tests;
