//! Settings → Advanced as the core decides it (`docs/settings.md`, `docs/mcp.md`): the local
//! server for AI assistants, then the reset of the local database. A client draws this and keeps
//! no rule of its own about it.

use crate::ControlState;

/// Settings → Advanced.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AdvancedPage {
    /// AI assistant access, drawn first; `None` where the host set no endpoint, so a phone has
    /// no such section.
    pub assistants: Option<McpPanel>,
    /// The button that resets the local database, after the person confirms it. Running from
    /// the confirmation until the cache is cleared, mail downloaded again and the file compacted.
    pub reset: ControlState,
    /// What the last reset came to, until the next one starts.
    pub reset_outcome: Option<ResetOutcome>,
}

/// What a reset of the local database came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ResetOutcome {
    /// The cache was cleared and mail downloaded again from every account that could be reached.
    Done,
    /// The local database could not be cleared, so the mail on this device is as it was.
    Failed,
}

/// AI assistant access: the local MCP server and what it may reach (`docs/mcp.md`).
///
/// Its order is the order of the decisions: turn it on, choose the mailboxes (none to begin
/// with), then, separately, let it send. The three below the switch are drawn only while it is on.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct McpPanel {
    /// Whether the person turned the server on. Off by default.
    pub enabled: bool,
    /// What the server is doing, said under the switch.
    pub status: McpStatus,
    /// Every account used for mail, with whether it is exposed.
    pub accounts: Vec<McpAccountRow>,
    /// No account is exposed: said in words, because an assistant reporting an empty inbox is
    /// otherwise indistinguishable from one given nothing to look at.
    pub nothing_exposed: bool,
    /// Whether an assistant may send mail directly, with no human review. Off by default.
    pub allow_direct_send: bool,
    /// Whether a direct send is restricted to people the person already writes to. On by default.
    pub require_known_recipient: bool,
    /// Whether the known-recipient switch can be changed: only while direct send is on. Drawn
    /// disabled rather than hidden otherwise, so the guard is seen before direct send is.
    pub known_recipient_editable: bool,
    /// The MCP-client configuration to show and offer to copy.
    pub snippet: String,
}

/// What the local server is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum McpStatus {
    /// The person has not turned it on.
    Off,
    /// Turned on; the endpoint is not bound yet.
    Starting,
    /// An assistant can connect.
    Listening,
    /// Another copy of the app is serving assistants, and this one stays off rather than take its
    /// endpoint.
    AnotherInstance,
    /// The endpoint cannot be used; the log says why.
    Unavailable,
}

/// One account in the panel: who it is, and whether the person exposed it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct McpAccountRow {
    /// The account's id, passed back in `McpIntent::SetAccountExposed`.
    pub account_id: String,
    /// The account's address, its label.
    pub email: String,
    /// Whether an assistant may see and act on this account. **False by default.**
    pub exposed: bool,
}

/// Where this host's server listens and how an assistant starts the relay that reaches it,
/// which [`crate::MailcalApp::set_mcp_host`] hands the core once. Finding the relay is the host's,
/// because only the host knows how it was installed (an app bundle, a Flatpak, a package).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct McpHost {
    /// A Unix socket path, or a `\\.\pipe\…` name on Windows.
    pub endpoint: String,
    /// The relay's executable, as an MCP client runs it.
    pub relay_command: String,
    /// The relay's own arguments, before the endpoint the core adds.
    pub relay_args: Vec<String>,
}
