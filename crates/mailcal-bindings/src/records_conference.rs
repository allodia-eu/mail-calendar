//! The FFI mirror of an event's meetings, as the detail view shows them
//! (`docs/conferencing.md`).
//!
//! Its own file because `records_calendar.rs` is close to the 500-line limit, and because a
//! meeting is one coherent part of the detail: the record, its dial-in rows, its state and the
//! `From` impls belong together.
//!
//! **Every text field here came from whoever wrote the event**, an invitation's sender
//! included. A client renders it as **text**, never markup, and opens `join_url` and a dial-in
//! `uri` only through the OS, never in a web view of its own.

use mailcal_account::{
    ConferenceState as AppConferenceState, DialIn as AppDialIn,
    EventConference as AppEventConference,
};

/// Where a meeting the calendar server makes stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ConferenceState {
    /// It can be joined, or its numbers dialled.
    Ready,
    /// The server was asked for it and has not made it yet: show that it is being prepared,
    /// and nothing to join.
    Preparing,
    /// The server was asked for it and could not make it.
    Failed,
}

/// A number to dial into a meeting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ConferenceDialIn {
    /// The number as the server wrote it, for showing.
    pub number: String,
    /// The `tel:` URI that dials it; open it through the OS.
    pub uri: String,
    /// The PIN or access code to type once connected, if the server gave one.
    pub pin: Option<String>,
}

/// One meeting on an event.
///
/// The value derives are for Linux, the one host written in Rust; Swift, Kotlin and C# generate
/// value types of their own.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EventConference {
    /// The service's name ("Microsoft Teams", "Zoom"), a brand name the same in every
    /// language; `None` when neither the server nor the link names one, and the client shows
    /// its own generic words instead.
    pub service: Option<String>,
    /// The `https` link the Join button opens through the OS, which hands it to the service's
    /// app where one is installed. `None` while the meeting is prepared, or when it has only
    /// numbers.
    pub join_url: Option<String>,
    /// The ID a person types to join from an app or a phone.
    pub meeting_id: Option<String>,
    /// The passcode the join link asks for.
    pub passcode: Option<String>,
    /// The numbers to dial in on, in the server's order.
    pub dial_in: Vec<ConferenceDialIn>,
    /// The server's or service's instructions for it, as plain text.
    pub notes: Option<String>,
    /// Where it stands.
    pub state: ConferenceState,
}

impl From<AppEventConference> for EventConference {
    fn from(conference: AppEventConference) -> Self {
        Self {
            service: conference.service,
            join_url: conference.join_url,
            meeting_id: conference.meeting_id,
            passcode: conference.passcode,
            dial_in: conference
                .dial_in
                .into_iter()
                .map(|AppDialIn { number, uri, pin }| ConferenceDialIn { number, uri, pin })
                .collect(),
            notes: conference.notes,
            state: match conference.state {
                AppConferenceState::Ready => ConferenceState::Ready,
                AppConferenceState::Preparing => ConferenceState::Preparing,
                AppConferenceState::Failed => ConferenceState::Failed,
            },
        }
    }
}
