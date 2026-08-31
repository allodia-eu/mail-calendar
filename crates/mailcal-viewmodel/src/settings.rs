//! The settings view-model: the active display timezone and any pending change.
//!
//! Unlike the mail/calendar snapshots, this projects the host app's own preference
//! state (not engine domain types): which zone the agenda is shown in, and, when the
//! device reports a different OS zone: the zone the host should prompt the user to
//! switch to. The product-core ([`mailcal_app`](../mailcal_app/index.html)) owns the
//! state machine; this is the immutable view of it a host renders.

use crate::FolderRole;

/// An immutable snapshot of the display-timezone setting for a host to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimeZoneSnapshot {
    /// The active display zone's IANA id (e.g. `Europe/Amsterdam`); what the agenda
    /// is ordered and localised in.
    pub active: String,
    /// A different zone the device most recently reported, awaiting the user's choice
    /// to adopt or dismiss it; `None` when the device matches the active zone. A host
    /// renders this as a "your timezone changed; update?" prompt.
    pub pending_device: Option<String>,
}

/// The default reply/forward quote style, as the host renders it in settings. Mirrors the
/// persisted `mailcal_account::QuoteStyle`, kept here so the view-model crate stays free of
/// an account-layer dependency. The host shows it as the default in app settings and seeds a
/// new reply with it (overridable per message in the composer when the user has turned that
/// on; see [`QuoteSettings::per_message`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuoteStyleKind {
    /// Indent the original in a blockquote under a one-line "On … wrote:" attribution.
    #[default]
    Indented,
    /// Divide the original off with a rule and a labelled `From:/Sent:/To:/Subject:` block.
    LineAndHeader,
}

/// An immutable snapshot of the reply/forward quoting settings for a host to render: the
/// app-level default style, and whether the composer offers a per-message override of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuoteSettings {
    /// The style a new reply or forward is seeded with.
    pub style: QuoteStyleKind,
    /// Whether the composer shows the style picker at all. Off by default: a reply just uses
    /// [`QuoteSettings::style`]. A host must not show the composer picker when this is false.
    pub per_message: bool,
}

/// One signature in the user's library, as a settings list or a composer picker renders it.
///
/// Metadata only: the body is fetched one at a time (`App::signature_html`) so a list of ten
/// signatures does not carry ten logos' worth of base64 across the FFI to draw ten names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureRow {
    /// The signature's opaque id; what an assignment and the body fetch name it by.
    pub id: String,
    /// The user's name for it ("Work", "Personal").
    pub name: String,
}

/// Which of an account's two signature slots is meant. Mirrors the persisted
/// `mailcal_account::SignatureSlot`, kept here so the view-model crate stays free of an
/// account-layer dependency (the same shape as [`SwipeDirection`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSlotKind {
    /// The signature a brand-new message opens with.
    NewMessage,
    /// The signature a reply or a forward opens with; one slot for both, as in Outlook.
    ReplyForward,
}

/// One account's signature assignment for the settings screen: which signature it uses for a
/// new message and which for a reply/forward. `None` in a slot means **no signature** there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountSignatureRow {
    /// The account's id (passed back to the setter).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// The signature id used for new messages, or `None`.
    pub new_message: Option<String>,
    /// The signature id used for replies and forwards, or `None`.
    pub reply_forward: Option<String>,
}

/// An immutable snapshot of the signatures surface for a host to render: the library in the
/// user's chosen order, plus one row per configured account with its two assignments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignaturesSnapshot {
    /// The library, in display order.
    pub signatures: Vec<SignatureRow>,
    /// One row per configured account.
    pub accounts: Vec<AccountSignatureRow>,
}

/// What a swipe across a message row does, as the host renders it in settings. Mirrors the
/// persisted `mailcal_account::SwipeAction`, kept here so the view-model crate stays free of an
/// account-layer dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwipeActionKind {
    /// Move the message to the account's Trash folder (recoverable).
    #[default]
    Delete,
    /// Move the message to the account's Archive folder.
    Archive,
    /// Flag (star) the message, leaving it in the list.
    Star,
}

/// Which swipe a [`SwipeActionKind`] is bound to: the two independently configurable
/// directions of a message row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    /// Swiping the row leftwards (toward the start edge).
    Left,
    /// Swiping the row rightwards (toward the end edge).
    Right,
}

/// An immutable snapshot of the per-direction swipe actions for a host to render, and to decide
/// what a completed swipe does. Both directions default to [`SwipeActionKind::Delete`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SwipeSettings {
    /// What a leftward swipe does.
    pub left: SwipeActionKind,
    /// What a rightward swipe does.
    pub right: SwipeActionKind,
}

/// How an account receives new mail, as the host renders it. Mirrors the persisted
/// `mailcal_account::SyncStrategy`, kept here so the view-model crate stays free of an
/// account-layer dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStrategyKind {
    /// Receive mail as it arrives via IMAP `IDLE` (offered only when the server supports
    /// it; see [`AccountSyncRow::idle_supported`]).
    Push,
    /// Check for new mail on a timer ([`AccountSyncRow::poll_interval_mins`]).
    Poll,
}

/// One folder of an account, with whether it is subscribed for push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFolderRow {
    /// The mailbox's provider key.
    pub key: String,
    /// The folder's **server** name. A client shows its own word for a role-bearing folder
    /// (`docs/folder-pane.md` rule 12), so this list reads the same as the folder pane.
    pub name: String,
    /// The folder's special role, or `None` for an ordinary custom folder: the same value
    /// [`FolderRow::role`](crate::FolderRow) carries, and for the same reason: it is what picks
    /// the label and the icon, never a test on the name.
    pub role: Option<FolderRole>,
    /// Whether this folder is watched for push (meaningful only under
    /// [`SyncStrategyKind::Push`]).
    pub subscribed: bool,
}

/// One account's synchronisation-behaviour row for the settings screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSyncRow {
    /// The account's id (passed back to the setters).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether the server advertises IMAP `IDLE`; gates whether a client offers the
    /// "receive emails as they come in" option at all.
    pub idle_supported: bool,
    /// The strategy currently in effect for the account.
    pub strategy: SyncStrategyKind,
    /// The poll interval in minutes (one of [`SyncSettingsSnapshot::poll_intervals`]).
    pub poll_interval_mins: u16,
    /// How far back this account syncs mail, as a month count (`0` = all mail); one of
    /// [`SyncSettingsSnapshot::sync_depths`]. Per-account: an account without its own override
    /// shows the product default here.
    pub sync_depth_months: u16,
    /// The largest message this account downloads in full during the background body warm, as a
    /// megabyte count (`0` = no limit); one of [`SyncSettingsSnapshot::message_size_limits_mb`].
    /// Per-account: an account without its own override shows the product default here, which
    /// differs between a computer and a phone.
    pub message_size_limit_mb: u16,
    /// Whether the maximum number of push folders is already subscribed: the signal for
    /// a client to disable further (unchecked) folder toggles.
    pub at_push_limit: bool,
    /// Every folder of the account, with its push-subscription state.
    pub folders: Vec<SyncFolderRow>,
}

/// An immutable snapshot of the per-account synchronisation-behaviour settings for a host
/// to render; one [`AccountSyncRow`] per configured account, plus the shared limits a
/// client uses to build its pickers without hardcoding them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSettingsSnapshot {
    /// One row per configured account.
    pub accounts: Vec<AccountSyncRow>,
    /// The maximum folders an account may watch for push (the same on every platform).
    pub max_push_folders: u8,
    /// The selectable poll intervals in minutes, in display order.
    pub poll_intervals: Vec<u16>,
    /// The selectable per-account sync-depth options as month counts, in display order
    /// (`0` = all mail): a client builds its fetch-depth picker from this.
    pub sync_depths: Vec<u16>,
    /// The selectable per-account message-size options as megabyte counts, in display order
    /// (`0` = no limit): a client builds its message-size picker from this.
    pub message_size_limits_mb: Vec<u16>,
}

/// One account, as the MCP settings panel lists it: who it is, and whether the user has exposed
/// it to assistants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAccountRow {
    /// The account's id (passed back to the setter).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether an MCP client may see and act on this account. **False by default**; turning the
    /// server on exposes nothing until the user ticks an account.
    pub exposed: bool,
}

/// An immutable snapshot of the local MCP (AI assistant access) settings for a host to render.
///
/// Desktop-only in practice: `endpoint` is `None` on a platform whose host passes none, which is
/// how mobile ends up with no server without a `#[cfg]` anywhere in the core.
///
/// Four booleans because the panel has four independent switches; grouping them into an enum
/// would lose combinations the user can actually set (on, one account, direct send on, guard
/// off) and would have to be expanded again at the one place it is rendered.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpSettings {
    /// Whether the local server is on. Off by default.
    pub enabled: bool,
    /// Whether it is actually listening right now. Distinct from [`Self::enabled`]: a user can
    /// have turned it on while the endpoint is unusable (another instance owns it, a path that
    /// will not bind), and a panel that showed only the toggle would be lying.
    pub running: bool,
    /// Every configured account, with whether it is exposed.
    pub accounts: Vec<McpAccountRow>,
    /// Whether an assistant may send mail directly, with no human review. Off by default; with
    /// it off the send tool does not exist at all.
    pub allow_direct_send: bool,
    /// Whether a direct send is restricted to people the user already emails. On by default.
    pub require_known_recipient: bool,
    /// Where the server listens, or `None` when this platform has no endpoint. A host renders it
    /// into the config snippet it offers to copy.
    pub endpoint: Option<String>,
}
