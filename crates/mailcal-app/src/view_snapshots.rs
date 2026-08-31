//! Snapshot pull-accessors and outgoing-send status.
//!
//! Split out of `lib.rs` to keep it under the size limit; an `impl App` block reusing the
//! runtime's fields. Each accessor returns the current immutable snapshot a host pulls after
//! the matching `surface_changed` signal.

use std::sync::atomic::Ordering;

use engine_api::Provider;
use mailcal_viewmodel::{CalendarSnapshot, MailboxListSnapshot, ReadingSnapshot, TimeZoneSnapshot};

use crate::{App, SendStatus};

impl<P: Provider> App<P> {
    /// The current mailbox-list snapshot (pulled after a `surface_changed` signal).
    #[must_use]
    pub fn mailbox_list(&self) -> MailboxListSnapshot {
        self.mailbox_list.get()
    }

    /// The current calendar agenda snapshot (pulled after a `Surface::Calendar` signal).
    #[must_use]
    pub fn calendar_list(&self) -> CalendarSnapshot {
        self.calendar.get()
    }

    /// The current reading-view snapshot (pulled after a `Surface::Reading` signal): the
    /// open message's key and its fetched, sanitised body.
    #[must_use]
    pub fn reading_view(&self) -> ReadingSnapshot {
        self.reading.get()
    }

    /// The current display-timezone setting (pulled after a `Surface::Settings` signal):
    /// the active zone and any pending device-zone change the host prompts on.
    #[must_use]
    pub fn timezone_settings(&self) -> TimeZoneSnapshot {
        self.timezone
            .lock()
            .expect("timezone mutex poisoned")
            .snapshot()
    }

    /// The current outgoing-send status (pulled after a [`Surface::Sending`](crate::Surface)
    /// signal).
    #[must_use]
    pub fn send_status(&self) -> SendStatus {
        self.send_status.get()
    }

    /// Records the outgoing-send status and signals [`Surface::Sending`] so a host can
    /// update its "sending…" → "sent" hint. Bumps and returns the new send-status
    /// generation, which the terminal auto-clear uses as its staleness guard.
    pub(crate) fn set_send_status(&self, status: SendStatus) -> u64 {
        self.send_status.publish(status);
        self.send_status_generation.fetch_add(1, Ordering::SeqCst) + 1
    }
}
