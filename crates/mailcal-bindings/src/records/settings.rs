//! The FFI record/enum mirror types a **Settings** surface renders: sync behaviour, quoting,
//! signatures, swipe actions, and local MCP (AI assistant) access.
//!
//! Split out of `records.rs` (its parent) to stay under the 500-line limit, that file keeps the
//! mailbox/reading/calendar rows a host draws the mail itself from. The line is drawn by which
//! screen shows them: everything here is reached through Settings, is written back through a
//! `set_*` call on the FFI object, and is pulled fresh on a `Surface::Settings` signal. Like its
//! parent, these derive the UniFFI scaffolding and are re-exported from `lib.rs`, so the
//! generated Swift/Kotlin/C# and every call site are unchanged by the split.

use crate::FolderRole;

/// How an account receives new mail (pulled in [`SyncSettingsSnapshot`]).
#[derive(uniffi::Enum)]
pub enum SyncStrategyKind {
    /// Receive mail as it arrives via IMAP `IDLE`; offered only when
    /// [`AccountSyncRow::idle_supported`].
    Push,
    /// Check for new mail on a timer ([`AccountSyncRow::poll_interval_mins`]).
    Poll,
}

/// The default reply/forward quote style (from [`crate::MailcalApp::quote_settings`]).
#[derive(uniffi::Enum)]
pub enum QuoteStyleKind {
    /// Indent the original in a blockquote under "On … wrote:".
    Indented,
    /// Divide the original off with a rule and a `From:/Sent:/To:/Subject:` header block.
    LineAndHeader,
}

/// The reply/forward quoting settings (from [`crate::MailcalApp::quote_settings`]): the
/// app-level default style, and whether the composer offers a per-message override of it.
#[derive(uniffi::Record)]
pub struct QuoteSettings {
    /// The style a new reply or forward is seeded with.
    pub style: QuoteStyleKind,
    /// Whether the composer shows the style picker. Off by default: a reply just uses `style`,
    /// and the client must not show the composer picker.
    pub per_message: bool,
}

/// One signature in the user's library (from [`crate::MailcalApp::signatures`]); metadata only.
/// A body is fetched one at a time with [`crate::MailcalApp::signature_html`], so a list of ten
/// signatures does not carry ten logos across the FFI to draw ten names.
#[derive(uniffi::Record)]
pub struct SignatureRow {
    /// The signature's opaque id; what an assignment and a body fetch name it by.
    pub id: String,
    /// The user's name for it ("Work", "Personal").
    pub name: String,
}

/// Which of an account's two signature slots is meant: the argument to
/// [`crate::MailcalApp::set_account_signature`] and [`crate::MailcalApp::resolve_signature`].
#[derive(uniffi::Enum)]
pub enum SignatureSlotKind {
    /// The signature a brand-new message opens with.
    NewMessage,
    /// The signature a reply or a forward opens with; one slot for both, as in Outlook.
    ReplyForward,
}

/// One account's signature assignment in the settings screen. A `None` slot means **no
/// signature** there; both `None` is how a user turns signatures off for that account.
#[derive(uniffi::Record)]
pub struct AccountSignatureRow {
    /// The account's id (passed back to `set_account_signature`).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// The signature id used for new messages, or `None`.
    pub new_message: Option<String>,
    /// The signature id used for replies and forwards, or `None`.
    pub reply_forward: Option<String>,
}

/// The signatures surface (pulled after a `Surface::Settings` signal): the library in the user's
/// order, plus one row per configured account.
#[derive(uniffi::Record)]
pub struct SignaturesSnapshot {
    /// The library, in display order.
    pub signatures: Vec<SignatureRow>,
    /// One row per configured account.
    pub accounts: Vec<AccountSignatureRow>,
}

/// A signature resolved for a composer (from [`crate::MailcalApp::resolve_signature`] /
/// [`crate::MailcalApp::signature_body`]): both bodies, so the host seeds the editor and the core
/// has the text/plain rendering to send alongside it.
#[derive(uniffi::Record)]
pub struct SignatureBody {
    /// The signature's id, so a host can show which one its picker has selected.
    pub id: String,
    /// The HTML fragment to seed into the editor.
    pub body_html: String,
    /// The plain-text rendering that accompanies it.
    pub body_plain: String,
}

/// What a swipe across a message row does (from [`crate::MailcalApp::swipe_settings`]). The two
/// directions are configured independently in the settings screen.
#[derive(uniffi::Enum)]
pub enum SwipeActionKind {
    /// Move the message to the account's Trash folder (recoverable).
    Delete,
    /// Move the message to the account's Archive folder.
    Archive,
    /// Flag (star) the message, leaving it in the list.
    Star,
}

/// Which swipe a [`SwipeActionKind`] is bound to: the argument to
/// [`crate::MailcalApp::set_swipe_action`].
#[derive(uniffi::Enum)]
pub enum SwipeDirection {
    /// Swiping the row leftwards (toward the start edge).
    Left,
    /// Swiping the row rightwards (toward the end edge).
    Right,
}

/// The per-direction swipe actions a host binds its message-row gestures to (pulled after a
/// `Surface::Settings` signal). Both default to [`SwipeActionKind::Delete`].
#[derive(uniffi::Record)]
pub struct SwipeSettings {
    /// What a leftward swipe does.
    pub left: SwipeActionKind,
    /// What a rightward swipe does.
    pub right: SwipeActionKind,
}

/// One folder of an account in the sync-settings screen, with its push-subscription state.
#[derive(uniffi::Record)]
pub struct SyncFolderRow {
    /// The mailbox's provider key (passed back to `set_push_folder`).
    pub key: String,
    /// The folder's **server** name; show your own word for a role-bearing folder, as the
    /// folder pane does (`docs/folder-pane.md`).
    pub name: String,
    /// The folder's special role, or `None` for an ordinary custom folder.
    pub role: Option<FolderRole>,
    /// Whether this folder is watched for push (meaningful only under
    /// [`SyncStrategyKind::Push`]).
    pub subscribed: bool,
}

/// One account's row in the synchronisation-behaviour settings screen.
#[derive(uniffi::Record)]
pub struct AccountSyncRow {
    /// The account's id (passed back to the setters).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether the server advertises IMAP `IDLE`, when `false`, a client hides the
    /// "receive emails as they come in" option and offers only interval polling.
    pub idle_supported: bool,
    /// The strategy currently in effect.
    pub strategy: SyncStrategyKind,
    /// The poll interval in minutes (one of [`SyncSettingsSnapshot::poll_intervals`]).
    pub poll_interval_mins: u16,
    /// How far back this account syncs mail, as a month count (`0` = all mail); one of
    /// [`SyncSettingsSnapshot::sync_depths`]. Per-account: an account with no override shows the
    /// product default here. Set via `MailcalApp::set_account_sync_depth`.
    pub sync_depth_months: u16,
    /// The largest message this account downloads in full in the background, as a megabyte
    /// count (`0` = no limit); one of [`SyncSettingsSnapshot::message_size_limits_mb`].
    /// Per-account: an account with no override shows the product default here, which differs
    /// between a computer and a phone. Set via `MailcalApp::set_account_message_size_limit`.
    pub message_size_limit_mb: u16,
    /// Whether the max push folders are already subscribed: a client disables further
    /// (unchecked) folder toggles when `true`.
    pub at_push_limit: bool,
    /// Every folder of the account, with its push-subscription state.
    pub folders: Vec<SyncFolderRow>,
}

/// An immutable snapshot of the per-account synchronisation-behaviour settings (pulled
/// after a `Surface::Settings` signal): one row per account, plus the shared limits a
/// client builds its pickers from.
#[derive(uniffi::Record)]
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

/// One account in the MCP settings panel: who it is, and whether the user exposed it.
#[derive(uniffi::Record)]
pub struct McpAccountRow {
    /// The account's id (passed back to `set_mcp_account_exposed`).
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// Whether an MCP client may see and act on this account. **False by default.**
    pub exposed: bool,
}

/// The local MCP (AI assistant access) settings, as a host renders them.
#[derive(uniffi::Record)]
pub struct McpSettings {
    /// Whether the user turned the local server on. Off by default.
    pub enabled: bool,
    /// Whether it is actually listening right now; distinct from `enabled`, because a user can
    /// have turned it on while the endpoint is unusable, and a panel that showed only the toggle
    /// would be lying.
    pub running: bool,
    /// Every configured account, with whether it is exposed.
    pub accounts: Vec<McpAccountRow>,
    /// Whether an assistant may send mail directly, with no human review. Off by default.
    pub allow_direct_send: bool,
    /// Whether a direct send is restricted to people the user already emails. On by default.
    pub require_known_recipient: bool,
    /// Where the server listens, or `None` when this platform has no endpoint (mobile). A host
    /// renders it into the MCP-client config snippet it offers to copy.
    pub endpoint: Option<String>,
}

impl From<mailcal_viewmodel::McpAccountRow> for McpAccountRow {
    fn from(row: mailcal_viewmodel::McpAccountRow) -> Self {
        Self {
            account_id: row.account_id,
            email: row.email,
            exposed: row.exposed,
        }
    }
}

impl From<mailcal_viewmodel::McpSettings> for McpSettings {
    fn from(settings: mailcal_viewmodel::McpSettings) -> Self {
        Self {
            enabled: settings.enabled,
            running: settings.running,
            accounts: settings.accounts.into_iter().map(Into::into).collect(),
            allow_direct_send: settings.allow_direct_send,
            require_known_recipient: settings.require_known_recipient,
            endpoint: settings.endpoint,
        }
    }
}
