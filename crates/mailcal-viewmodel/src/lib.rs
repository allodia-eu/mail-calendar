//! `mailcal-viewmodel`: the pure view-models for Allodia Mail & Calendar.
//!
//! Immutable snapshots a host renders, projected from the engine's domain types
//! ([`engine_api::Message`]/[`engine_api::Mailbox`]/[`engine_api::Event`]). They carry no
//! runtime, no engine handle, and no FFI; state lives in the engine, grouping and
//! ordering live here, so both native renderers (SwiftUI/Compose) share one
//! definition. [`mailcal_app`](../mailcal_app/index.html) drives the
//! engine and calls [`view::build`]/[`calendar::build`] to refresh these.

pub mod attendee;
pub mod avatar;
pub mod calendar;
pub mod color;
pub mod connectivity;
pub mod contacts;
mod folders;
pub mod invitation;
pub mod reading;
pub mod settings;
pub mod sync_progress;
pub mod text;
pub mod view;
mod view_rows;

pub use attendee::{EventAttendee, effective_response, event_attendees};
pub use avatar::Avatar;
pub use calendar::{CalendarSnapshot, EventRow};
pub use connectivity::ConnectivitySnapshot;
pub use contacts::{ContactCardRef, ContactDetail, ContactRow, ContactValue, ContactsSnapshot};
pub use folders::{
    AccountFolderRow, FolderRole, FolderRow, folder_role, inbox_unread, sorted_folder_rows,
};
pub use invitation::{AttendeeTally, InvitationCard, InvitationKind, ResponseStatus};
pub use reading::{AttachmentRow, ReadingSnapshot};
pub use settings::{
    AccountSignatureRow, AccountSyncRow, DefaultMailAppOutcome, DefaultMailAppSupport,
    McpAccountRow, McpSettings, QuoteSettings, QuoteStyleKind, SignatureRow, SignatureSlotKind,
    SignaturesSnapshot, SwipeActionKind, SwipeDirection, SwipeSettings, SyncFolderRow,
    SyncSettingsSnapshot, SyncStrategyKind, TimeZoneSnapshot,
};
pub use sync_progress::{AccountSyncProgress, SyncProgressSnapshot};
pub use text::plain_text;
pub use view::{
    AccountMessage, AccountRow, FlatRow, MailboxListSnapshot, SearchHorizon, SnapshotRow,
    ThreadMessage, ThreadRow, ViewMode,
};
