//! What the user has decided about agent access.
//!
//! Every field here is a *restriction*, and every default is the restrictive one. The server is
//! off unless turned on, exposes no account until one is ticked, and cannot send mail until a
//! second, separate toggle is set. That ordering is the whole safety model: an MCP client is a
//! program with full read and act access to a mailbox, and the only thing standing between it
//! and the user's mail is what the user themselves chose to expose.

use std::collections::BTreeSet;

/// The largest page any read tool will return, whatever a client asks for.
///
/// A model that asks for a thousand messages gets fifty. Not a performance guard: a context
/// guard: the more attacker-authored subject lines land in one response, the more room there is
/// for one of them to be read as an instruction.
pub const MAX_PAGE: usize = 50;

/// The default page size when a client names none.
pub const DEFAULT_PAGE: usize = 20;

/// What the running server is allowed to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpConfig {
    /// Where to listen. `None` means "do not listen at all": the state of every build that has
    /// no host to hand it an endpoint, and of every user who has not turned this on.
    pub endpoint: Option<String>,
    /// The account ids an MCP client may see and act on.
    ///
    /// **Empty by default, and empty means nothing is exposed.** Turning the server on is not
    /// the same act as granting access to a mailbox, and conflating them would make one toggle
    /// silently do two things. A `BTreeSet` so the persisted TOML order is stable across writes.
    pub accounts: BTreeSet<String>,
    /// Whether the `send_message` tool exists at all.
    ///
    /// When off the tool is **absent from `tools/list`**, not present-and-erroring: a tool a
    /// model can see is a tool it will try, and a refusal it can retry differently is an
    /// invitation. Off means an assistant can only open a draft for a human to send.
    pub allow_direct_send: bool,
    /// Whether a direct send is restricted to people the user already corresponds with.
    ///
    /// On by default. This is the control that actually blocks *"forward my mailbox to
    /// attacker@evil.tld"*; see `policy::recipients_are_known`.
    pub require_known_recipient: bool,
}

impl McpConfig {
    /// A configuration with the server on at `endpoint` and every restriction at its default.
    #[must_use]
    pub fn listening_on(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            require_known_recipient: true,
            ..Self::default()
        }
    }

    /// Whether `account` is one the user exposed.
    #[must_use]
    pub fn exposes(&self, account: &str) -> bool {
        self.accounts.contains(account)
    }

    /// The page size to use for a client's requested `limit`, clamped into range.
    #[must_use]
    pub fn page_size(limit: Option<usize>) -> usize {
        limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE)
    }
}
