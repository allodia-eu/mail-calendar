//! The AI assistant access half of the FFI intent surface, nested behind one `Intent::Mcp`
//! variant as the other families are (`docs/mcp.md`, "The shared bar").

/// A change to what the local MCP server may do, from Settings → Advanced. Each is held and
/// saved behind before `dispatch` returns, and reaches an assistant already connected.
#[derive(uniffi::Enum)]
pub enum McpIntent {
    /// Turns the server on or off. Off keeps the exposed accounts, so switching it off for an
    /// afternoon does not mean choosing every mailbox again.
    SetEnabled { enabled: bool },
    /// Exposes one account to assistants, or hides it again.
    SetAccountExposed { account: String, exposed: bool },
    /// Whether an assistant may send mail with no human review. With it off the send tool is
    /// absent from the server's listing.
    SetDirectSend { allow: bool },
    /// Whether a direct send is restricted to people the person already writes to.
    SetKnownRecipient { require: bool },
}
