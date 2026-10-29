//! The routes an answer can take, as the account's capabilities decide them: the shapes
//! `InvitationFake` takes beyond its server-scheduled default.

use engine_provider::{Capabilities, OverrideSurvival, WriteGuard};

use super::{InvitationFake, SERVER_SCHEDULED};

impl InvitationFake {
    /// A plain RFC 4791 calendar beside a mail account that *can* send an iMIP message; the
    /// IMAP-plus-CalDAV shape, and the one the whole client-iMIP route exists for. The meeting
    /// is on no calendar, because nothing put it there.
    pub(crate) fn without_server_scheduling(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_scheduling_submission()
            .with_calendars()
            .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
            .with_calendar_rsvp(SERVER_SCHEDULED);
        self.materialized = false;
        self
    }

    /// The same, on a calendar that holds the meeting but cannot store an answer to it: the
    /// reply is the whole answer.
    pub(crate) fn without_stored_answers(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_scheduling_submission()
            .with_calendars()
            .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept());
        self.controls = None;
        self
    }

    /// The same, on an account with **no calendar at all**: a bare mailbox. Nothing can be
    /// stored, and the reply is the entire answer.
    ///
    /// Drops the calendar provider from the account rather than only clearing its capabilities,
    /// because the two are different states and the code distinguishes them: a provider that
    /// advertises no calendar is still a provider a write can be attempted against, while an
    /// account with no `calendar_provider` has nothing to attempt at all. Modelling only
    /// the first would let a store slip through that a real bare mailbox could never reach.
    pub(crate) fn with_mail_only(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_scheduling_submission();
        self.controls = None;
        self.materialized = false;
        self.has_calendar = false;
        self
    }

    /// A mail transport that cannot put `method=` on a body part (JMAP), beside a calendar
    /// server that does not schedule: no route at all.
    pub(crate) fn with_no_route(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_message_source()
            .with_submission()
            .with_calendars()
            .with_calendar_writes(WriteGuard::Enforced, OverrideSurvival::kept())
            .with_calendar_rsvp(SERVER_SCHEDULED);
        self
    }
}
